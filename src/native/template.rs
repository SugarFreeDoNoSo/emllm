use super::{BaseRef, NativeError, NativeErrorKind, NativeLimits, Phase, PreparedBase, Resource};
use crate::{model_io::F32Bits, synth::structural::Builder};
use std::{collections::HashMap, sync::Arc};

/// Índice de base, puerto o instrucción anterior, con procedencia de plantilla.
#[derive(Clone, Debug)]
pub struct TemplateRef {
    identity: Arc<()>,
    index: usize,
}
impl TemplateRef {
    pub fn index(&self) -> usize {
        self.index
    }
}
impl PartialEq for TemplateRef {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for TemplateRef {}

/// Plantilla inmutable; sus puertos no son nodos del lenguaje cerrado.
/// No puede pasarse a un evaluador sin asociar e instanciar las dependencias.
///
/// ```compile_fail
/// use emllm::{eval::evaluate, native::Template};
/// fn read_template(template: &Template) { evaluate(template, 100); }
/// ```
///
/// ```compile_fail
/// use emllm::native::TemplateRef;
/// let reference = TemplateRef { identity: Default::default(), index: 99 };
/// ```
#[derive(Debug)]
pub struct Template {
    identity: Arc<()>,
    base: Arc<PreparedBase>,
    ports: usize,
    pairs: Vec<[usize; 2]>,
    roots: Vec<usize>,
    built_instructions: usize,
    construction_index_capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateStats {
    pub base_nodes: usize,
    pub ports: usize,
    pub instructions: usize,
    pub built_instructions: usize,
    pub roots: usize,
    pub pair_payload_bytes: usize,
    pub pair_capacity_bytes: usize,
    pub root_payload_bytes: usize,
    pub root_capacity_bytes: usize,
    /// Histórico del constructor liberado, no memoria residente de Template.
    pub construction_index_capacity: usize,
    pub construction_index_modeled_payload_bytes: usize,
    pub metadata_bytes: usize,
}

impl Template {
    pub fn base(&self) -> &Arc<PreparedBase> {
        &self.base
    }
    pub fn ports(&self) -> usize {
        self.ports
    }
    /// Rangos: base `[0,B)`, puertos `[B,B+P)`, instrucción i en `B+P+i`.
    pub fn instruction_offset(&self) -> usize {
        self.base.program().node_count() + self.ports
    }
    pub fn pairs(&self) -> &[[usize; 2]] {
        &self.pairs
    }
    pub fn root_indices(&self) -> &[usize] {
        &self.roots
    }
    pub fn roots(&self) -> impl ExactSizeIterator<Item = TemplateRef> + '_ {
        self.roots.iter().map(|&index| TemplateRef {
            identity: self.identity.clone(),
            index,
        })
    }
    pub fn stats(&self) -> TemplateStats {
        TemplateStats {
            base_nodes: self.base.program().node_count(),
            ports: self.ports,
            instructions: self.pairs.len(),
            built_instructions: self.built_instructions,
            roots: self.roots.len(),
            pair_payload_bytes: size_of_val(self.pairs.as_slice()),
            pair_capacity_bytes: self.pairs.capacity() * size_of::<[usize; 2]>(),
            root_payload_bytes: size_of_val(self.roots.as_slice()),
            root_capacity_bytes: self.roots.capacity() * size_of::<usize>(),
            construction_index_capacity: self.construction_index_capacity,
            construction_index_modeled_payload_bytes: self.construction_index_capacity
                * size_of::<([usize; 2], usize)>(),
            metadata_bytes: size_of::<Self>(),
        }
    }
}

/// Constructor estructural sin tokens. Conserva dimensiones, presupuesto e identidad.
pub struct TemplateBuilder {
    identity: Arc<()>,
    base: Arc<PreparedBase>,
    ports: usize,
    offset: usize,
    pairs: Vec<[usize; 2]>,
    interned: HashMap<[usize; 2], usize>,
    limits: NativeLimits,
}

impl TemplateBuilder {
    pub fn new(
        base: Arc<PreparedBase>,
        ports: usize,
        limits: NativeLimits,
    ) -> Result<Self, NativeError> {
        limits.check(Phase::Template, Resource::Ports, ports)?;
        let offset = base
            .program()
            .node_count()
            .checked_add(ports)
            .ok_or_else(|| {
                NativeError::overflow(Phase::Template, Resource::TemplateInstructions)
            })?;
        Ok(Self {
            identity: Arc::new(()),
            base,
            ports,
            offset,
            pairs: Vec::new(),
            interned: HashMap::new(),
            limits,
        })
    }
    pub fn port(&self, index: usize) -> Result<TemplateRef, NativeError> {
        if index >= self.ports {
            return Err(error(NativeErrorKind::InvalidReference { index }));
        }
        Ok(self.handle(self.base.program().node_count() + index))
    }
    pub fn base_reference(&self, reference: &BaseRef) -> Result<TemplateRef, NativeError> {
        let index = self.base.validate(reference).map_err(|mut e| {
            e.phase = Phase::Template;
            e
        })?;
        Ok(self.handle(index))
    }
    pub fn one(&mut self) -> Result<TemplateRef, NativeError> {
        <Self as Builder>::one(self)
    }
    pub fn constant(&mut self, bits: F32Bits) -> Result<TemplateRef, NativeError> {
        <Self as Builder>::constant(self, bits)
    }
    pub fn magma(
        &mut self,
        left: &TemplateRef,
        right: &TemplateRef,
    ) -> Result<TemplateRef, NativeError> {
        <Self as Builder>::magma(self, left, right)
    }
    pub fn instruction_count(&self) -> usize {
        self.pairs.len()
    }
    fn handle(&self, index: usize) -> TemplateRef {
        TemplateRef {
            identity: self.identity.clone(),
            index,
        }
    }
    fn validate(&self, reference: &TemplateRef) -> Result<usize, NativeError> {
        if !Arc::ptr_eq(&self.identity, &reference.identity) {
            return Err(error(NativeErrorKind::ForeignReference));
        }
        if reference.index >= self.offset + self.pairs.len() {
            return Err(error(NativeErrorKind::InvalidReference {
                index: reference.index,
            }));
        }
        Ok(reference.index)
    }

    /// Recorta a la clausura estructural de las raíces. Libera el internado antes
    /// de reservar el remapeo; compacta in-place sin copiar otra tabla de pares.
    pub fn finish(mut self, roots: &[TemplateRef]) -> Result<Template, NativeError> {
        if roots.is_empty() {
            return Err(error(NativeErrorKind::EmptyRoots));
        }
        self.limits
            .check(Phase::Template, Resource::Roots, roots.len())?;
        for root in roots {
            self.validate(root)?;
        }
        validate_pairs(&self.pairs, self.offset)?;
        let index_capacity = self.interned.capacity();
        drop(self.interned);
        let built = self.pairs.len();
        let mut remap = Vec::new();
        self.limits.reserve(
            Phase::Template,
            Resource::TemplateInstructions,
            &mut remap,
            built,
        )?;
        remap.resize(built, usize::MAX);
        for root in roots {
            if root.index >= self.offset {
                remap[root.index - self.offset] = 0;
            }
        }
        for i in (0..built).rev() {
            if remap[i] != usize::MAX {
                for child in self.pairs[i] {
                    if child >= self.offset {
                        remap[child - self.offset] = 0;
                    }
                }
            }
        }
        let mut count = 0;
        for i in 0..built {
            if remap[i] != usize::MAX {
                let [left, right] = self.pairs[i].map(|child| {
                    if child < self.offset {
                        child
                    } else {
                        remap[child - self.offset]
                    }
                });
                remap[i] = self.offset + count;
                self.pairs[count] = [left, right];
                count += 1;
            }
        }
        self.pairs.truncate(count);
        let mut closed_roots = Vec::new();
        self.limits.reserve(
            Phase::Template,
            Resource::Roots,
            &mut closed_roots,
            roots.len(),
        )?;
        closed_roots.extend(roots.iter().map(|root| {
            if root.index < self.offset {
                root.index
            } else {
                remap[root.index - self.offset]
            }
        }));
        Ok(Template {
            identity: Arc::new(()),
            base: self.base,
            ports: self.ports,
            pairs: self.pairs,
            roots: closed_roots,
            built_instructions: built,
            construction_index_capacity: index_capacity,
        })
    }
}

impl Builder for TemplateBuilder {
    type Ref = TemplateRef;
    type Error = NativeError;
    fn one(&mut self) -> Result<TemplateRef, NativeError> {
        Ok(self.handle(0))
    }
    fn constant(&mut self, bits: F32Bits) -> Result<TemplateRef, NativeError> {
        let reference = self
            .base
            .constant(bits)
            .ok_or_else(|| error(NativeErrorKind::MissingConstant { bits: bits.bits() }))?;
        self.base_reference(&reference)
    }
    fn magma(
        &mut self,
        left: &TemplateRef,
        right: &TemplateRef,
    ) -> Result<TemplateRef, NativeError> {
        let pair = [self.validate(left)?, self.validate(right)?];
        let base_nodes = self.base.program().node_count();
        if pair.iter().all(|&i| i < base_nodes)
            && let Some(reference) = self
                .base
                .find_pair(&self.base.handle(pair[0]), &self.base.handle(pair[1]))?
        {
            return Ok(self.handle(reference.index()));
        }
        if let Some(&index) = self.interned.get(&pair) {
            return Ok(self.handle(index));
        }
        let index = self
            .offset
            .checked_add(self.pairs.len())
            .filter(|&i| i < usize::MAX)
            .ok_or_else(|| {
                NativeError::overflow(Phase::Template, Resource::TemplateInstructions)
            })?;
        self.limits.reserve(
            Phase::Template,
            Resource::TemplateInstructions,
            &mut self.pairs,
            1,
        )?;
        self.limits.reserve_map(
            Phase::Template,
            Resource::TemplateInstructions,
            &mut self.interned,
            1,
        )?;
        self.pairs.push(pair);
        self.interned.insert(pair, index);
        Ok(self.handle(index))
    }
}

fn error(kind: NativeErrorKind) -> NativeError {
    NativeError {
        phase: Phase::Template,
        kind,
    }
}
fn validate_pairs(pairs: &[[usize; 2]], offset: usize) -> Result<(), NativeError> {
    let total = offset
        .checked_add(pairs.len())
        .ok_or_else(|| NativeError::overflow(Phase::Template, Resource::TemplateInstructions))?;
    for (i, pair) in pairs.iter().enumerate() {
        for &child in pair {
            if child >= total {
                return Err(error(NativeErrorKind::InvalidReference { index: child }));
            }
            if child >= offset + i {
                return Err(error(NativeErrorKind::TemplateTopology {
                    instruction: offset + i,
                    operand: child,
                }));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::structural;

    fn base() -> Arc<PreparedBase> {
        Arc::new(
            PreparedBase::from_constants(
                &[F32Bits::new(0x3f80_0000).unwrap()],
                NativeLimits::default(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn repeated_ports_constants_and_roots_share_without_becoming_closed_programs() {
        let base = base();
        let nodes = base.program().node_count();
        let mut builder = TemplateBuilder::new(base.clone(), 2, NativeLimits::default()).unwrap();
        let a = builder.port(0).unwrap();
        assert_eq!(a, builder.port(0).unwrap());
        let one = builder.one().unwrap();
        let root = structural::exp(&mut builder, &a).unwrap();
        assert_eq!(builder.magma(&a, &one).unwrap(), root);
        let base_value = base.constant(F32Bits::new(0x3f80_0000).unwrap()).unwrap();
        let constant = builder.base_reference(&base_value).unwrap();
        assert_eq!(
            constant,
            builder
                .constant(F32Bits::new(0x3f80_0000).unwrap())
                .unwrap()
        );
        let b = builder.port(1).unwrap();
        builder.magma(&b, &a).unwrap(); // No alcanzable desde las raíces.
        let template = builder.finish(&[root.clone(), constant, root]).unwrap();
        assert!(Arc::ptr_eq(&base, template.base()));
        assert_eq!(template.pairs(), &[[nodes, 0]]);
        assert_eq!(template.root_indices()[0], template.root_indices()[2]);
        assert_eq!(template.stats().built_instructions, 2);
        assert_eq!(template.stats().instructions, 1);
        assert_eq!(base.program().node_count(), nodes);
    }

    #[test]
    fn provenance_limits_and_missing_constants_are_rejected_before_insertion() {
        let base = base();
        let limits = NativeLimits {
            template_instructions: 1,
            roots: 1,
            ports: 1,
            ..NativeLimits::default()
        };
        let mut builder = TemplateBuilder::new(base.clone(), 1, limits).unwrap();
        let other = TemplateBuilder::new(base.clone(), 1, limits).unwrap();
        let a = builder.port(0).unwrap();
        let foreign = other.port(0).unwrap();
        assert_eq!(a.index(), foreign.index());
        assert_eq!(
            builder.magma(&a, &foreign).unwrap_err().kind,
            NativeErrorKind::ForeignReference
        );
        assert!(
            builder
                .constant(F32Bits::new(0x4000_0000).unwrap())
                .is_err()
        );
        let first = builder.magma(&a, &a).unwrap();
        assert_eq!(first, builder.magma(&a, &a).unwrap());
        assert!(builder.magma(&first, &a).is_err());
        assert_eq!(builder.instruction_count(), 1);
        assert!(builder.finish(&[first.clone(), first]).is_err());
        assert!(TemplateBuilder::new(base.clone(), 2, limits).is_err());
        let mut zero = TemplateBuilder::new(
            base,
            0,
            NativeLimits {
                template_instructions: 0,
                ..limits
            },
        )
        .unwrap();
        assert!(zero.port(0).is_err());
        let one = zero.one().unwrap();
        zero.finish(&[one]).unwrap();
    }

    #[test]
    fn malformed_internal_fixtures_cannot_publish_templates() {
        assert!(matches!(
            validate_pairs(&[[2, 0]], 1).unwrap_err().kind,
            NativeErrorKind::InvalidReference { index: 2 }
        ));
        for pairs in [vec![[1, 0]], vec![[2, 0], [1, 0]]] {
            assert!(matches!(
                validate_pairs(&pairs, 1).unwrap_err().kind,
                NativeErrorKind::TemplateTopology { .. }
            ));
        }
        let mut builder = TemplateBuilder::new(base(), 1, NativeLimits::default()).unwrap();
        let port = builder.port(0).unwrap();
        builder.pairs.push([builder.offset, 0]);
        assert!(builder.finish(&[port]).is_err()); // Incluso una instrucción inválida no alcanzable.
    }
}
