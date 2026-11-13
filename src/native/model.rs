//! Ensamblador estructural del modelo fijado. La preparación solo recibe base,
//! longitud y selección diagnóstica; los IDs llegan posteriormente a compose.
use super::{
    NativeError, NativeErrorKind, NativeLimits, Phase, PreparedBase, PreparedModel, Resource,
    TemplateBuilder, TemplateRef,
    attention::Rope,
    components::RmsNorm,
    tensor::{Shape, Tensor, TensorBuilder, TensorView},
};
use crate::model_io::{F32Bits, TensorKind};
use std::{ops::Range, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageLayout {
    RowMajor,
    Causal,
}

#[derive(Clone, Debug)]
pub struct Stage {
    pub name: String,
    pub shape: Shape,
    pub roots: Range<usize>,
    pub layout: StageLayout,
}

impl Stage {
    /// Coordenadas del tensor de referencia, incluida la coordenada temporal
    /// compacta de scores/probabilidades. No ejecuta expresiones.
    pub fn coordinates(&self, root: usize) -> Option<Vec<usize>> {
        if !self.roots.contains(&root) {
            return None;
        }
        let mut flat = root - self.roots.start;
        let mut coordinates = vec![0; self.shape.dimensions().len()];
        for (coordinate, dimension) in coordinates.iter_mut().zip(self.shape.dimensions()).rev() {
            *coordinate = flat % dimension;
            flat /= dimension;
        }
        Some(coordinates)
    }
    pub fn position_and_key(&self, root: usize) -> Option<(usize, Option<usize>)> {
        let coordinates = self.coordinates(root)?;
        if self.layout == StageLayout::Causal {
            let mut packed = coordinates[1];
            let mut position = 0;
            while packed > position {
                packed -= position + 1;
                position += 1;
            }
            Some((position, Some(packed)))
        } else {
            Some((coordinates[0], None))
        }
    }
}

pub struct PreparedBlock {
    model: PreparedModel,
    stages: Vec<Stage>,
    diagnostic: bool,
}
impl PreparedBlock {
    pub fn prepare(
        base: Arc<PreparedBase>,
        length: usize,
        diagnostic: bool,
        limits: NativeLimits,
    ) -> Result<Self, NativeError> {
        validate_model(&base, length)?;
        let ports = limits.checked_elements(Phase::Template, Resource::Ports, &[length, 64])?;
        let mut builder = TemplateBuilder::new(base, ports, limits)?;
        let mut input = Vec::new();
        limits.reserve(Phase::Template, Resource::Ports, &mut input, ports)?;
        for index in 0..ports {
            input.push(builder.port(index)?);
        }
        let input = Tensor::from_vec(
            Shape::new(&[length, 64]).expect("fixed positive shape"),
            input,
        )
        .expect("port shape");
        let mut trace = Trace {
            roots: Vec::new(),
            stages: Vec::new(),
            diagnostic,
            limits,
        };
        trace.record("embedding", &input.view(), StageLayout::RowMajor)?;
        let output = block(&mut builder, &input.view(), 0, &mut trace)?;
        if !diagnostic {
            trace.push("layers.0.residual", &output.view(), StageLayout::RowMajor)?;
        }
        let template = builder.finish(&trace.roots)?;
        Ok(Self {
            model: PreparedModel::from_template(template, length)?,
            stages: trace.stages,
            diagnostic,
        })
    }
    pub fn model(&self) -> &PreparedModel {
        &self.model
    }
    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }
    pub fn diagnostic(&self) -> bool {
        self.diagnostic
    }
}

fn validate_model(base: &PreparedBase, length: usize) -> Result<(), NativeError> {
    let valid = base.config().is_some_and(|c| {
        c.dim == 64
            && c.hidden_dim == 172
            && c.n_layers == 5
            && c.n_heads == 8
            && c.n_kv_heads == 4
            && c.vocab_size == 512
            && c.rope_positions as usize >= length
    });
    if !valid || ![8, 16].contains(&length) {
        return Err(NativeError {
            phase: Phase::Template,
            kind: NativeErrorKind::ModelConfiguration,
        });
    }
    Ok(())
}

struct Trace {
    roots: Vec<TemplateRef>,
    stages: Vec<Stage>,
    diagnostic: bool,
    limits: NativeLimits,
}
impl Trace {
    fn record(
        &mut self,
        name: &str,
        tensor: &TensorView<'_, TemplateRef>,
        layout: StageLayout,
    ) -> Result<(), NativeError> {
        if self.diagnostic {
            self.push(name, tensor, layout)?;
        }
        Ok(())
    }
    fn push(
        &mut self,
        name: &str,
        tensor: &TensorView<'_, TemplateRef>,
        layout: StageLayout,
    ) -> Result<(), NativeError> {
        let start = self.roots.len();
        self.limits.reserve(
            Phase::Template,
            Resource::Roots,
            &mut self.roots,
            tensor.as_slice().len(),
        )?;
        self.limits
            .reserve(Phase::Template, Resource::Roots, &mut self.stages, 1)?;
        self.roots.extend_from_slice(tensor.as_slice());
        self.stages.push(Stage {
            name: name.into(),
            shape: tensor.shape(),
            roots: start..self.roots.len(),
            layout,
        });
        Ok(())
    }
    fn norm(&mut self, name: &str, norm: &RmsNorm<TemplateRef>) -> Result<(), NativeError> {
        for (suffix, tensor) in [
            ("squares", &norm.squares),
            ("mean", &norm.mean),
            ("denominator", &norm.denominator),
            ("inverse", &norm.inverse),
            ("normalized", &norm.normalized),
        ] {
            self.record(
                &format!("{name}.{suffix}"),
                &tensor.view(),
                StageLayout::RowMajor,
            )?;
        }
        self.record(name, &norm.output.view(), StageLayout::RowMajor)
    }
    fn rope(&mut self, name: &str, rope: &Rope<TemplateRef>) -> Result<(), NativeError> {
        for (suffix, tensor) in [
            ("real_cos", &rope.real_cos),
            ("imag_sin", &rope.imag_sin),
            ("real_sin", &rope.real_sin),
            ("imag_cos", &rope.imag_cos),
        ] {
            self.record(
                &format!("{name}.{suffix}"),
                &tensor.view(),
                StageLayout::RowMajor,
            )?;
        }
        self.record(name, &rope.output.view(), StageLayout::RowMajor)
    }
}

fn weight(
    builder: &TemplateBuilder,
    kind: TensorKind,
    layer: Option<u32>,
) -> Result<Tensor<TemplateRef>, NativeError> {
    let index = builder
        .base()
        .tensors()
        .iter()
        .position(|t| t.kind() == kind && t.layer() == layer)
        .ok_or(NativeError {
            phase: Phase::Template,
            kind: NativeErrorKind::ModelConfiguration,
        })?;
    builder.base_tensor(index)
}

fn block(
    builder: &mut TemplateBuilder,
    input: &TensorView<'_, TemplateRef>,
    layer: u32,
    trace: &mut Trace,
) -> Result<Tensor<TemplateRef>, NativeError> {
    let t = input.shape().dimensions()[0];
    let p = format!("layers.{layer}.");
    let epsilon = weight(builder, TensorKind::Epsilon, None)?;
    let scale = builder.constant(F32Bits::new(0x3c800000).unwrap())?;
    let norm_weight = weight(builder, TensorKind::AttentionNorm, Some(layer))?;
    let norm = builder.rmsnorm(
        input,
        norm_weight.as_slice(),
        &epsilon.as_slice()[0],
        &scale,
    )?;
    trace.norm(&format!("{p}attention_norm"), &norm)?;
    let q_weight = weight(builder, TensorKind::Query, Some(layer))?;
    let k_weight = weight(builder, TensorKind::Key, Some(layer))?;
    let v_weight = weight(builder, TensorKind::Value, Some(layer))?;
    let q = builder
        .project(&norm.output.view(), &q_weight.view())?
        .reshape(Shape::new(&[t, 8, 8]).unwrap())
        .unwrap();
    let k = builder
        .project(&norm.output.view(), &k_weight.view())?
        .reshape(Shape::new(&[t, 4, 8]).unwrap())
        .unwrap();
    let v = builder
        .project(&norm.output.view(), &v_weight.view())?
        .reshape(Shape::new(&[t, 4, 8]).unwrap())
        .unwrap();
    for (name, tensor) in [("query", &q), ("key", &k), ("value", &v)] {
        trace.record(&format!("{p}{name}"), &tensor.view(), StageLayout::RowMajor)?;
    }
    let cos = weight(builder, TensorKind::RopeCos, None)?;
    let sin = weight(builder, TensorKind::RopeSin, None)?;
    let cos = TensorView::new(Shape::new(&[t, 4]).unwrap(), &cos.as_slice()[..t * 4]).unwrap();
    let sin = TensorView::new(Shape::new(&[t, 4]).unwrap(), &sin.as_slice()[..t * 4]).unwrap();
    let q = builder.rope(&q.view(), &cos, &sin)?;
    let k = builder.rope(&k.view(), &cos, &sin)?;
    trace.rope(&format!("{p}query_rope"), &q)?;
    trace.rope(&format!("{p}key_rope"), &k)?;
    let attention_scale = weight(builder, TensorKind::AttentionScale, None)?;
    let attention = builder.attention(
        &q.output.view(),
        &k.output.view(),
        &v.view(),
        &attention_scale.as_slice()[0],
    )?;
    for (name, tensor) in [
        ("scores", &attention.scores),
        ("score_shifts", &attention.shifts),
        ("score_exp", &attention.exponentials),
        ("probabilities", &attention.probabilities),
    ] {
        trace.record(&format!("{p}{name}"), &tensor.view(), StageLayout::Causal)?;
    }
    trace.record(
        &format!("{p}context"),
        &attention.context.view(),
        StageLayout::RowMajor,
    )?;
    let output_weight = weight(builder, TensorKind::AttentionOutput, Some(layer))?;
    let attention_output = builder.project(&attention.context.view(), &output_weight.view())?;
    trace.record(
        &format!("{p}attention_output"),
        &attention_output.view(),
        StageLayout::RowMajor,
    )?;
    let h = builder.residual(input, &attention_output.view())?;
    trace.record(
        &format!("{p}attention_residual"),
        &h.view(),
        StageLayout::RowMajor,
    )?;
    let norm_weight = weight(builder, TensorKind::FeedForwardNorm, Some(layer))?;
    let norm = builder.rmsnorm(
        &h.view(),
        norm_weight.as_slice(),
        &epsilon.as_slice()[0],
        &scale,
    )?;
    trace.norm(&format!("{p}ffn_norm"), &norm)?;
    let gate_weight = weight(builder, TensorKind::Gate, Some(layer))?;
    let up_weight = weight(builder, TensorKind::Up, Some(layer))?;
    let gate = builder.project(&norm.output.view(), &gate_weight.view())?;
    let up = builder.project(&norm.output.view(), &up_weight.view())?;
    trace.record(&format!("{p}gate"), &gate.view(), StageLayout::RowMajor)?;
    trace.record(&format!("{p}up"), &up.view(), StageLayout::RowMajor)?;
    let swiglu = builder.swiglu(&gate.view(), &up.view())?;
    for (name, tensor) in [
        ("negative_gate", &swiglu.negative_gate),
        ("gate_exp", &swiglu.gate_exp),
        ("gate_denominator", &swiglu.gate_denominator),
        ("silu", &swiglu.silu),
        ("gated", &swiglu.gated),
    ] {
        trace.record(&format!("{p}{name}"), &tensor.view(), StageLayout::RowMajor)?;
    }
    let down_weight = weight(builder, TensorKind::Down, Some(layer))?;
    let down = builder.project(&swiglu.gated.view(), &down_weight.view())?;
    trace.record(&format!("{p}down"), &down.view(), StageLayout::RowMajor)?;
    let output = builder.residual(&h.view(), &down.view())?;
    trace.record(
        &format!("{p}residual"),
        &output.view(),
        StageLayout::RowMajor,
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_roots_have_checked_coordinates_and_a_distinct_template_identity() {
        let base = Arc::new(PreparedBase::vocabulary_fixture(8, 2));
        let limits = NativeLimits::default();
        let mut identities = Vec::new();
        let mut stats = Vec::new();
        for diagnostic in [false, true] {
            let mut builder = TemplateBuilder::new(base.clone(), 6, limits).unwrap();
            let mut trace = Trace {
                roots: Vec::new(),
                stages: Vec::new(),
                diagnostic,
                limits,
            };
            let ports: Vec<_> = (0..6).map(|i| builder.port(i).unwrap()).collect();
            let view = TensorView::new(Shape::new(&[1, 6]).unwrap(), &ports).unwrap();
            trace.record("scores", &view, StageLayout::Causal).unwrap();
            let root = builder.magma(&ports[0], &ports[1]).unwrap();
            let value =
                TensorView::new(Shape::new(&[1, 1]).unwrap(), std::slice::from_ref(&root)).unwrap();
            trace
                .push("residual", &value, StageLayout::RowMajor)
                .unwrap();
            if diagnostic {
                assert_eq!(trace.stages[0].position_and_key(4), Some((2, Some(1))));
                assert!(trace.stages[0].coordinates(6).is_none());
            }
            let template = builder.finish(&trace.roots).unwrap();
            identities.push(template.roots().last().unwrap());
            stats.push(template.stats());
        }
        assert_ne!(identities[0], identities[1]);
        assert_eq!(stats[0].roots, 1);
        assert_eq!(stats[1].roots, 7);
        assert!(PreparedBlock::prepare(base, 8, true, limits).is_err());
    }
}
