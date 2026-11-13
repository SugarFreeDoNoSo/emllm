use super::{
    CompositionBuilder, NativeError, NativeErrorKind, NativeLimits, NativeOutput, NativePrefix,
    Phase, PreparedBase, Resource, Template,
};
use crate::model_io::TensorKind;
use std::sync::Arc;

/// Modelo estructural parametrizado por una matriz de embeddings y una plantilla.
/// El ensamblador del Transformer determina fórmulas/raíces; esta frontera no
/// recibe tokens ni promete que una plantilla arbitraria sea stories260K completo.
#[derive(Debug)]
pub struct PreparedModel {
    template: Template,
    length: usize,
    embedding: usize,
    channels: usize,
}

impl PreparedModel {
    pub fn from_template(template: Template, length: usize) -> Result<Self, NativeError> {
        let embedding = template
            .base()
            .tensors()
            .iter()
            .position(|t| t.kind() == TensorKind::Embedding && t.layer().is_none())
            .ok_or_else(|| failure(Phase::Template, NativeErrorKind::MissingEmbedding))?;
        let shape = template.base().tensors()[embedding].shape();
        if shape.len() != 2 || length == 0 {
            return Err(failure(
                Phase::Template,
                NativeErrorKind::Shape {
                    expected: 2,
                    actual: shape.len(),
                },
            ));
        }
        let channels = shape[1];
        let expected = length
            .checked_mul(channels)
            .ok_or_else(|| NativeError::overflow(Phase::Template, Resource::Ports))?;
        if template.ports() != expected {
            return Err(failure(
                Phase::Template,
                NativeErrorKind::Shape {
                    expected,
                    actual: template.ports(),
                },
            ));
        }
        Ok(Self {
            template,
            length,
            embedding,
            channels,
        })
    }
    pub fn base(&self) -> &Arc<PreparedBase> {
        self.template.base()
    }
    pub fn template(&self) -> &Template {
        &self.template
    }
    pub fn length(&self) -> usize {
        self.length
    }

    /// Cronometrar toda esta llamada: valida, crea estado nuevo, selecciona filas,
    /// asocia puertos, instancia/interna y materializa raíces. Libera temporales
    /// antes de retornar. No usa evaluador ni salidas de solicitudes anteriores.
    pub fn compose(
        &self,
        prefix: &NativePrefix,
        limits: NativeLimits,
    ) -> Result<NativeOutput, NativeError> {
        prefix
            .validate_for(self.base(), self.length)
            .map_err(|mut e| {
                e.phase = Phase::Compose;
                e
            })?;
        limits.check(
            Phase::Compose,
            Resource::Roots,
            self.template.root_indices().len(),
        )?;
        let mut request = CompositionBuilder::new(self.base().clone(), limits)?;
        let mut ports = Vec::new();
        let count = limits.checked_elements(
            Phase::Compose,
            Resource::Ports,
            &[self.length, self.channels],
        )?;
        limits.reserve(Phase::Compose, Resource::Ports, &mut ports, count)?;
        for symbol in prefix.symbols() {
            let start = (symbol.token_id() as usize)
                .checked_mul(self.channels)
                .ok_or_else(|| NativeError::overflow(Phase::Compose, Resource::Ports))?;
            for channel in 0..self.channels {
                let (_, reference) = self
                    .base()
                    .tensor_scalar(self.embedding, start + channel)
                    .ok_or_else(|| {
                        failure(
                            Phase::Compose,
                            NativeErrorKind::TokenRange {
                                id: symbol.token_id(),
                            },
                        )
                    })?;
                ports.push(request.base_reference(&reference)?.index());
            }
        }
        let base_nodes = self.base().program().node_count();
        let offset = self.template.instruction_offset();
        let mut remap: Vec<usize> = Vec::new();
        limits.reserve(
            Phase::Compose,
            Resource::TemplateInstructions,
            &mut remap,
            self.template.pairs().len(),
        )?;
        let resolve = |index: usize, remap: &[usize]| -> Result<usize, NativeError> {
            if index < base_nodes {
                return Ok(index);
            }
            let result = if index < offset {
                ports.get(index - base_nodes)
            } else {
                remap.get(index - offset)
            };
            result
                .copied()
                .ok_or_else(|| failure(Phase::Compose, NativeErrorKind::InvalidReference { index }))
        };
        for &[left, right] in self.template.pairs() {
            let left = request.reference_at(resolve(left, &remap)?)?;
            let right = request.reference_at(resolve(right, &remap)?)?;
            remap.push(request.magma(&left, &right)?.index());
        }
        let mut roots = Vec::new();
        limits.reserve(
            Phase::Compose,
            Resource::Roots,
            &mut roots,
            self.template.root_indices().len(),
        )?;
        for &index in self.template.root_indices() {
            roots.push(request.reference_at(resolve(index, &remap)?)?);
        }
        request.finish(&roots)
    }
}

fn failure(phase: Phase, kind: NativeErrorKind) -> NativeError {
    NativeError { phase, kind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::evaluate, native::TemplateBuilder, program::compile_nodes, synth::structural,
    };

    fn model() -> PreparedModel {
        let base = Arc::new(PreparedBase::vocabulary_fixture(8, 2));
        let mut builder = TemplateBuilder::new(base, 4, NativeLimits::default()).unwrap();
        let a = builder.port(0).unwrap();
        let b = builder.port(2).unwrap();
        let root = structural::add(&mut builder, &a, &b).unwrap();
        PreparedModel::from_template(builder.finish(&[root]).unwrap(), 2).unwrap()
    }

    fn read(output: &NativeOutput) -> Vec<f64> {
        let program = compile_nodes(|i| output.node(i), output.root_indices(), 10000).unwrap();
        evaluate(&program, 10000).unwrap()
    }

    #[test]
    fn a_b_failed_a_use_one_immutable_preparation_and_new_requests() {
        let model = model();
        let base_stats = model.base().storage();
        let template_stats = model.template().stats();
        let limits = NativeLimits::default();
        let a = NativePrefix::encode(model.base().clone(), &[1, 2], &[0, 1], 2, limits).unwrap();
        let b = NativePrefix::encode(model.base().clone(), &[1, 3], &[0, 1], 2, limits).unwrap();
        let first = model.compose(&a, limits).unwrap();
        let second = model.compose(&b, limits).unwrap();
        assert!((read(&first)[0] - 3.0).abs() < 1e-8);
        assert!((read(&second)[0] - 2.0).abs() < 1e-8);
        let failure = model
            .compose(
                &a,
                NativeLimits {
                    closed_nodes: model.base().program().node_count(),
                    ..limits
                },
            )
            .unwrap_err();
        assert_eq!(failure.phase, Phase::Compose);
        let third = model.compose(&a, limits).unwrap();
        assert_eq!(first.delta_pairs(), third.delta_pairs());
        assert_eq!(first.root_indices(), third.root_indices());
        assert_eq!(first.storage(), third.storage());
        assert_ne!(first.roots().next(), third.roots().next());
        assert_eq!(read(&first), read(&third));
        assert_eq!(model.base().storage(), base_stats);
        assert_eq!(model.template().stats(), template_stats);
        assert!(Arc::ptr_eq(first.base(), model.base()));
        drop(model);
        assert!((read(&first)[0] - 3.0).abs() < 1e-8);
    }

    #[test]
    fn foreign_prefix_and_bad_embedding_port_shape_are_rejected() {
        let model = model();
        let other = Arc::new(PreparedBase::vocabulary_fixture(8, 2));
        let foreign =
            NativePrefix::encode(other, &[1, 2], &[0, 1], 2, NativeLimits::default()).unwrap();
        assert_eq!(
            model
                .compose(&foreign, NativeLimits::default())
                .unwrap_err()
                .kind,
            NativeErrorKind::ForeignReference
        );
        let builder =
            TemplateBuilder::new(model.base().clone(), 1, NativeLimits::default()).unwrap();
        let root = builder.port(0).unwrap();
        assert!(PreparedModel::from_template(builder.finish(&[root]).unwrap(), 2).is_err());
    }
}
