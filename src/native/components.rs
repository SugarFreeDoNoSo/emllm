//! Componentes tensoriales construidos con las expansiones compartidas.
use super::{
    CompositionBuilder, NativeError, NativeRef, TemplateBuilder, TemplateRef,
    tensor::{self, Shape, Tensor, TensorBuilder, TensorView},
};
use crate::synth::structural::{self, model};

/// Intermedios cerrados/formales para diagnóstico, con el mismo layout de entrada.
pub struct RmsNorm<R> {
    pub squares: Tensor<R>,
    pub mean: Tensor<R>,
    pub denominator: Tensor<R>,
    pub inverse: Tensor<R>,
    pub normalized: Tensor<R>,
    pub output: Tensor<R>,
}

pub struct SwiGlu<R> {
    pub negative_gate: Tensor<R>,
    pub gate_exp: Tensor<R>,
    pub gate_denominator: Tensor<R>,
    pub silu: Tensor<R>,
    pub gated: Tensor<R>,
}

pub(crate) fn rmsnorm<B: TensorBuilder>(
    builder: &mut B,
    input: &TensorView<'_, B::Ref>,
    weight: &[B::Ref],
    epsilon: &B::Ref,
    mean_scale: &B::Ref,
) -> Result<RmsNorm<B::Ref>, NativeError> {
    let [rows, channels] = tensor::matrix(input.shape()).map_err(|e| builder.tensor_error(e))?;
    tensor::check_length(channels, weight.len()).map_err(|e| builder.tensor_error(e))?;
    builder.validate_slice(input.as_slice())?;
    builder.validate_slice(weight)?;
    builder.validate_reference(epsilon)?;
    builder.validate_reference(mean_scale)?;
    let mut squares = Vec::new();
    let mut mean = Vec::new();
    let mut denominator = Vec::new();
    let mut inverse = Vec::new();
    let mut normalized = Vec::new();
    let mut output = Vec::new();
    for values in [&mut squares, &mut normalized, &mut output] {
        builder.reserve(values, input.as_slice().len())?;
    }
    for values in [&mut mean, &mut denominator, &mut inverse] {
        builder.reserve(values, rows)?;
    }
    for row in 0..rows {
        let values = input.row(row).expect("bounded row");
        for value in values {
            squares.push(model::mul(builder, value, value)?);
        }
        let total = tensor::balanced_sum(builder, &squares[row * channels..(row + 1) * channels])?;
        let m = model::mul(builder, &total, mean_scale)?;
        let d = model::add(builder, &m, epsilon)?;
        let inv = model::inverse_sqrt(builder, &d)?;
        for (value, w) in values.iter().zip(weight) {
            let n = model::mul(builder, value, &inv)?;
            output.push(model::mul(builder, &n, w)?);
            normalized.push(n);
        }
        mean.push(m);
        denominator.push(d);
        inverse.push(inv);
    }
    let reduced = Shape::new(&[rows, 1]).map_err(|e| builder.tensor_error(e))?;
    Ok(RmsNorm {
        squares: Tensor::from_vec(input.shape(), squares).expect("known shape"),
        mean: Tensor::from_vec(reduced, mean).expect("known shape"),
        denominator: Tensor::from_vec(reduced, denominator).expect("known shape"),
        inverse: Tensor::from_vec(reduced, inverse).expect("known shape"),
        normalized: Tensor::from_vec(input.shape(), normalized).expect("known shape"),
        output: Tensor::from_vec(input.shape(), output).expect("known shape"),
    })
}

pub(crate) fn swiglu<B: TensorBuilder>(
    builder: &mut B,
    gate: &TensorView<'_, B::Ref>,
    up: &TensorView<'_, B::Ref>,
) -> Result<SwiGlu<B::Ref>, NativeError> {
    tensor::check_shape(gate.shape(), up.shape()).map_err(|e| builder.tensor_error(e))?;
    builder.validate_slice(gate.as_slice())?;
    builder.validate_slice(up.as_slice())?;
    let mut negative_gate = Vec::new();
    let mut gate_exp = Vec::new();
    let mut gate_denominator = Vec::new();
    let mut silu = Vec::new();
    let mut gated = Vec::new();
    for values in [
        &mut negative_gate,
        &mut gate_exp,
        &mut gate_denominator,
        &mut silu,
        &mut gated,
    ] {
        builder.reserve(values, gate.as_slice().len())?;
    }
    for (g, u) in gate.as_slice().iter().zip(up.as_slice()) {
        let parts = model::silu_parts(builder, g)?;
        gate_exp.push(structural::exp(builder, &parts.negative)?);
        gated.push(model::mul(builder, &parts.activated, u)?);
        negative_gate.push(parts.negative);
        gate_denominator.push(parts.denominator);
        silu.push(parts.activated);
    }
    let wrap = |v| Tensor::from_vec(gate.shape(), v).expect("known shape");
    Ok(SwiGlu {
        negative_gate: wrap(negative_gate),
        gate_exp: wrap(gate_exp),
        gate_denominator: wrap(gate_denominator),
        silu: wrap(silu),
        gated: wrap(gated),
    })
}

macro_rules! component_methods {
    ($builder:ty, $reference:ty) => {
        impl $builder {
            /// `mean_scale` representa 1/dimensión real; stories260K usa el F32
            /// exacto 1/64 ya preparado. Epsilon procede del paquete fijado.
            pub fn rmsnorm(
                &mut self,
                input: &TensorView<'_, $reference>,
                weight: &[$reference],
                epsilon: &$reference,
                mean_scale: &$reference,
            ) -> Result<RmsNorm<$reference>, NativeError> {
                rmsnorm(self, input, weight, epsilon, mean_scale)
            }
            pub fn swiglu(
                &mut self,
                gate: &TensorView<'_, $reference>,
                up: &TensorView<'_, $reference>,
            ) -> Result<SwiGlu<$reference>, NativeError> {
                swiglu(self, gate, up)
            }
        }
    };
}
component_methods!(TemplateBuilder, TemplateRef);
component_methods!(CompositionBuilder, NativeRef);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::probe,
        model_io::F32Bits,
        native::{NativeErrorKind, NativeLimits, Phase, PreparedBase, decode},
    };
    use std::sync::Arc;

    #[test]
    fn component_domains_and_provenance_are_not_fixed_by_construction() {
        let limits = NativeLimits::default();
        let bits: Vec<_> = [
            0.0f32,
            1.0,
            -1.0,
            2.0,
            128.0,
            1.0 / 64.0,
            1.0 / 8.0,
            4096.0,
            512.0,
            0.5,
            64.0,
        ]
        .into_iter()
        .map(|v| F32Bits::new(v.to_bits()).unwrap())
        .collect();
        let base = Arc::new(PreparedBase::from_constants(&bits, limits).unwrap());
        let mut builder = CompositionBuilder::new(base.clone(), limits).unwrap();
        let zero = builder
            .base_reference(&base.constant(bits[0]).unwrap())
            .unwrap();
        let one = builder.one().unwrap();
        let mut other = CompositionBuilder::new(base, limits).unwrap();
        let foreign = other.one().unwrap();
        let gates = vec![one.clone(); 4];
        let matrix = TensorView::new(Shape::new(&[2, 2]).unwrap(), &gates).unwrap();
        let row = TensorView::new(Shape::new(&[1, 4]).unwrap(), &gates).unwrap();
        assert!(builder.swiglu(&matrix, &row).is_err());
        let input =
            TensorView::new(Shape::new(&[1, 1]).unwrap(), std::slice::from_ref(&zero)).unwrap();
        assert!(
            builder
                .rmsnorm(&input, std::slice::from_ref(&one), &foreign, &one)
                .is_err()
        );
        let (result, calls) = probe::observe(true, || {
            builder
                .rmsnorm(&input, std::slice::from_ref(&one), &zero, &one)
                .unwrap()
        });
        assert_eq!(calls, Default::default());
        let output = builder.finish(result.output.as_slice()).unwrap();
        let failure = decode(&output, limits).unwrap_err();
        assert_eq!(failure.phase, Phase::EvaluateOriginal);
        assert!(matches!(failure.kind, NativeErrorKind::Evaluation(_)));
    }
}
