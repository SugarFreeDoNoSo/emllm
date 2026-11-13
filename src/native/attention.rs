//! RoPE real y atención temporal causal. El empaquetado de canales no convierte
//! scores/probabilidades en números complejos ni introduce términos enmascarados.
use super::{
    CompositionBuilder, NativeError, NativeRef, TemplateBuilder, TemplateRef,
    tensor::{self, Shape, Tensor, TensorBuilder, TensorError, TensorView},
};
use crate::synth::structural::{self, model};

pub struct Rope<R> {
    pub real_cos: Tensor<R>,
    pub imag_sin: Tensor<R>,
    pub real_sin: Tensor<R>,
    pub imag_cos: Tensor<R>,
    pub output: Tensor<R>,
}
pub struct Softmax<R> {
    pub shifts: Vec<R>,
    pub exponentials: Vec<R>,
    pub probabilities: Vec<R>,
}
pub struct Attention<R> {
    /// `[Q_heads, T*(T+1)/2]`, posiciones concatenadas dentro de cada cabeza.
    pub scores: Tensor<R>,
    pub shifts: Tensor<R>,
    pub exponentials: Tensor<R>,
    pub probabilities: Tensor<R>,
    /// `[T, Q_heads*head_dim]`.
    pub context: Tensor<R>,
}

fn heads(shape: Shape) -> Result<[usize; 3], TensorError> {
    if let [positions, heads, channels] = *shape.dimensions() {
        Ok([positions, heads, channels])
    } else {
        Err(TensorError::Rank {
            actual: shape.dimensions().len(),
        })
    }
}

pub(crate) fn rope<B: TensorBuilder>(
    builder: &mut B,
    input: &TensorView<'_, B::Ref>,
    cos: &TensorView<'_, B::Ref>,
    sin: &TensorView<'_, B::Ref>,
) -> Result<Rope<B::Ref>, NativeError> {
    let [positions, count, dim] = heads(input.shape()).map_err(|e| builder.tensor_error(e))?;
    if dim % 2 != 0 {
        return Err(builder.tensor_error(TensorError::Dimension {
            axis: 2,
            expected: dim - 1,
            actual: dim,
        }));
    }
    let coefficients = Shape::new(&[positions, dim / 2]).map_err(|e| builder.tensor_error(e))?;
    tensor::check_shape(coefficients, cos.shape()).map_err(|e| builder.tensor_error(e))?;
    tensor::check_shape(coefficients, sin.shape()).map_err(|e| builder.tensor_error(e))?;
    for values in [input.as_slice(), cos.as_slice(), sin.as_slice()] {
        builder.validate_slice(values)?;
    }
    let mut real_cos = Vec::new();
    let mut imag_sin = Vec::new();
    let mut real_sin = Vec::new();
    let mut imag_cos = Vec::new();
    for values in [&mut real_cos, &mut imag_sin, &mut real_sin, &mut imag_cos] {
        builder.reserve(values, input.as_slice().len() / 2)?;
    }
    let mut output = Vec::new();
    builder.reserve(&mut output, input.as_slice().len())?;
    for position in 0..positions {
        for head in 0..count {
            for pair in 0..dim / 2 {
                let a = input
                    .get(&[position, head, 2 * pair])
                    .expect("bounded channel");
                let b = input
                    .get(&[position, head, 2 * pair + 1])
                    .expect("bounded channel");
                let c = cos.get(&[position, pair]).expect("bounded coefficient");
                let s = sin.get(&[position, pair]).expect("bounded coefficient");
                let ac = model::mul(builder, a, c)?;
                let bs = model::mul(builder, b, s)?;
                let as_ = model::mul(builder, a, s)?;
                let bc = model::mul(builder, b, c)?;
                output.push(model::sub(builder, &ac, &bs)?);
                output.push(model::add(builder, &as_, &bc)?);
                real_cos.push(ac);
                imag_sin.push(bs);
                real_sin.push(as_);
                imag_cos.push(bc);
            }
        }
    }
    let product_shape =
        Shape::new(&[positions, count, dim / 2]).map_err(|e| builder.tensor_error(e))?;
    let wrap = |values| Tensor::from_vec(product_shape, values).expect("known product shape");
    Ok(Rope {
        real_cos: wrap(real_cos),
        imag_sin: wrap(imag_sin),
        real_sin: wrap(real_sin),
        imag_cos: wrap(imag_cos),
        output: Tensor::from_vec(input.shape(), output).expect("known rotation shape"),
    })
}

pub(crate) fn softmax<B: TensorBuilder>(
    builder: &mut B,
    scores: &[B::Ref],
) -> Result<Softmax<B::Ref>, NativeError> {
    let pivot = scores
        .first()
        .ok_or_else(|| builder.tensor_error(TensorError::EmptyDimension { axis: 0 }))?;
    builder.validate_slice(scores)?;
    let mut shifts = Vec::new();
    let mut exponentials = Vec::new();
    let mut probabilities = Vec::new();
    let mut terms = Vec::new();
    for values in [
        &mut shifts,
        &mut exponentials,
        &mut probabilities,
        &mut terms,
    ] {
        builder.reserve(values, scores.len())?;
    }
    for score in scores {
        let shift = model::sub(builder, score, pivot)?;
        exponentials.push(structural::exp(builder, &shift)?);
        shifts.push(shift);
    }
    terms.extend_from_slice(&exponentials);
    let total = tensor::reduce(builder, terms, model::add_positive)?;
    for exponential in &exponentials {
        probabilities.push(model::divide_positive(builder, exponential, &total)?);
    }
    Ok(Softmax {
        shifts,
        exponentials,
        probabilities,
    })
}

pub(crate) fn attention<B: TensorBuilder>(
    builder: &mut B,
    query: &TensorView<'_, B::Ref>,
    key: &TensorView<'_, B::Ref>,
    value: &TensorView<'_, B::Ref>,
    scale: &B::Ref,
) -> Result<Attention<B::Ref>, NativeError> {
    let [positions, q_heads, dim] = heads(query.shape()).map_err(|e| builder.tensor_error(e))?;
    let [key_positions, kv_heads, key_dim] =
        heads(key.shape()).map_err(|e| builder.tensor_error(e))?;
    tensor::check_shape(key.shape(), value.shape()).map_err(|e| builder.tensor_error(e))?;
    tensor::check_length(positions, key_positions).map_err(|e| builder.tensor_error(e))?;
    tensor::check_length(dim, key_dim).map_err(|e| builder.tensor_error(e))?;
    if q_heads % kv_heads != 0 {
        return Err(builder.tensor_error(TensorError::Dimension {
            axis: 1,
            expected: kv_heads,
            actual: q_heads,
        }));
    }
    builder.validate_reference(scale)?;
    for values in [query.as_slice(), key.as_slice(), value.as_slice()] {
        builder.validate_slice(values)?;
    }
    let triangular = positions
        .checked_add(1)
        .and_then(|n| n.checked_mul(positions))
        .map(|n| n / 2)
        .ok_or_else(|| builder.tensor_error(TensorError::SizeOverflow))?;
    let score_shape = Shape::new(&[q_heads, triangular]).map_err(|e| builder.tensor_error(e))?;
    let context_shape =
        Shape::new(&[positions, q_heads, dim]).map_err(|e| builder.tensor_error(e))?;
    let mut scores = Vec::new();
    let mut shifts = Vec::new();
    let mut exponentials = Vec::new();
    let mut probabilities = Vec::new();
    for values in [
        &mut scores,
        &mut shifts,
        &mut exponentials,
        &mut probabilities,
    ] {
        builder.reserve(values, score_shape.elements())?;
    }
    let mut context = Vec::new();
    builder.reserve(&mut context, context_shape.elements())?;
    context.resize(context_shape.elements(), builder.one()?);
    let group = q_heads / kv_heads;
    for head in 0..q_heads {
        let kv_head = head / group;
        for position in 0..positions {
            let row_start = scores.len();
            let q_start = query
                .shape()
                .index(&[position, head, 0])
                .expect("bounded query");
            let q = &query.as_slice()[q_start..q_start + dim];
            for previous in 0..=position {
                let k_start = key
                    .shape()
                    .index(&[previous, kv_head, 0])
                    .expect("bounded key");
                let score = tensor::dot(builder, q, &key.as_slice()[k_start..k_start + dim])?;
                scores.push(model::mul(builder, &score, scale)?);
            }
            let soft = softmax(builder, &scores[row_start..])?;
            let mut values = Vec::new();
            builder.reserve(&mut values, position + 1)?;
            for channel in 0..dim {
                values.clear();
                for previous in 0..=position {
                    values.push(
                        value
                            .get(&[previous, kv_head, channel])
                            .expect("bounded value")
                            .clone(),
                    );
                }
                let index = context_shape
                    .index(&[position, head, channel])
                    .expect("bounded context");
                context[index] = tensor::dot(builder, &soft.probabilities, &values)?;
            }
            shifts.extend(soft.shifts);
            exponentials.extend(soft.exponentials);
            probabilities.extend(soft.probabilities);
        }
    }
    let wrap = |values| Tensor::from_vec(score_shape, values).expect("known causal shape");
    Ok(Attention {
        scores: wrap(scores),
        shifts: wrap(shifts),
        exponentials: wrap(exponentials),
        probabilities: wrap(probabilities),
        context: Tensor::from_vec(
            Shape::new(&[positions, q_heads * dim]).expect("checked context"),
            context,
        )
        .expect("known context shape"),
    })
}

macro_rules! attention_methods {
    ($builder:ty, $reference:ty) => {
        impl $builder {
            pub fn rope(
                &mut self,
                input: &TensorView<'_, $reference>,
                cos: &TensorView<'_, $reference>,
                sin: &TensorView<'_, $reference>,
            ) -> Result<Rope<$reference>, NativeError> {
                rope(self, input, cos, sin)
            }
            pub fn softmax(
                &mut self,
                scores: &[$reference],
            ) -> Result<Softmax<$reference>, NativeError> {
                softmax(self, scores)
            }
            pub fn attention(
                &mut self,
                query: &TensorView<'_, $reference>,
                key: &TensorView<'_, $reference>,
                value: &TensorView<'_, $reference>,
                scale: &$reference,
            ) -> Result<Attention<$reference>, NativeError> {
                attention(self, query, key, value, scale)
            }
        }
    };
}
attention_methods!(TemplateBuilder, TemplateRef);
attention_methods!(CompositionBuilder, NativeRef);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::probe,
        model_io::F32Bits,
        native::{NativeLimits, PreparedBase, decode},
    };
    use std::sync::Arc;

    fn close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (index, (&a, &b)) in actual.iter().zip(expected).enumerate() {
            assert!(
                (a - b).abs() <= 1e-6 + 1e-5 * b.abs(),
                "{index}: {a} != {b}"
            );
        }
    }

    #[test]
    fn attention_rejects_incompatible_heads_coefficients_and_provenance() {
        let limits = NativeLimits::default();
        let base = Arc::new(PreparedBase::vocabulary_fixture(8, 1));
        let mut builder = TemplateBuilder::new(base.clone(), 1, limits).unwrap();
        let other = TemplateBuilder::new(base, 1, limits).unwrap();
        let local = builder.port(0).unwrap();
        let foreign = other.port(0).unwrap();
        assert!(builder.softmax(&[]).is_err());
        assert!(builder.softmax(&[foreign]).is_err());
        let q_values = vec![local.clone(); 3 * 8];
        let k_values = vec![local.clone(); 2 * 8];
        let q = TensorView::new(Shape::new(&[1, 3, 8]).unwrap(), &q_values).unwrap();
        let k = TensorView::new(Shape::new(&[1, 2, 8]).unwrap(), &k_values).unwrap();
        assert!(builder.attention(&q, &k, &k, &local).is_err());
        let coeff = TensorView::new(Shape::new(&[1, 2]).unwrap(), &k_values[..2]).unwrap();
        assert!(builder.rope(&q, &coeff, &coeff).is_err());
        assert_eq!(builder.instruction_count(), 0);
    }

    #[test]
    fn real_rope_gqa_and_causality_match_independent_f64() {
        let limits = NativeLimits::default();
        let q: Vec<f32> = (0..3 * 8 * 8)
            .map(|i| ((i * 7 % 17) as f32 - 8.0) / 32.0)
            .collect();
        let k: Vec<f32> = (0..3 * 4 * 8)
            .map(|i| ((i * 3 % 11) as f32 - 5.0) / 32.0)
            .collect();
        let v: Vec<f32> = (0..3 * 4 * 8)
            .map(|i| (i % 13) as f32 / 16.0 - 0.25)
            .collect();
        let cos: Vec<f32> = (0..12).map(|i| (i as f32 * 0.2).cos()).collect();
        let sin: Vec<f32> = (0..12).map(|i| (i as f32 * 0.2).sin()).collect();
        let scale = (8.0f32).sqrt().recip();
        let fixed = [
            0.0,
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
            scale,
        ];
        let mut changed = v.clone();
        for value in &mut changed[64..] {
            *value += 1.0;
        }
        let bits: Vec<_> = [&q[..], &k, &v, &cos, &sin, &fixed, &changed]
            .into_iter()
            .flatten()
            .map(|n| F32Bits::new(n.to_bits()).unwrap())
            .collect();
        let base = Arc::new(PreparedBase::from_constants(&bits, limits).unwrap());
        let rotate = |x: &[f32], heads: usize| {
            let mut result: Vec<f64> = Vec::new();
            for position in 0..3 {
                for head in 0..heads {
                    for pair in 0..4 {
                        let i = (position * heads + head) * 8 + 2 * pair;
                        let (a, b, c, s) = (
                            f64::from(x[i]),
                            f64::from(x[i + 1]),
                            f64::from(cos[position * 4 + pair]),
                            f64::from(sin[position * 4 + pair]),
                        );
                        result.extend([a * c - b * s, a * s + b * c]);
                    }
                }
            }
            result
        };
        let qr = rotate(&q, 8);
        let kr = rotate(&k, 4);
        let mut expected_scores = Vec::new();
        let mut expected_probabilities = Vec::new();
        let mut expected_context = vec![0.0; 3 * 64];
        for head in 0..8 {
            for position in 0..3 {
                let scores: Vec<_> = (0..=position)
                    .map(|t| {
                        (0..8)
                            .map(|c| {
                                qr[(position * 8 + head) * 8 + c] * kr[(t * 4 + head / 2) * 8 + c]
                            })
                            .sum::<f64>()
                            * f64::from(scale)
                    })
                    .collect();
                let exp: Vec<_> = scores.iter().map(|s| (s - scores[0]).exp()).collect();
                let sum: f64 = exp.iter().sum();
                let probability: Vec<_> = exp.iter().map(|e| e / sum).collect();
                for c in 0..8 {
                    expected_context[position * 64 + head * 8 + c] = probability
                        .iter()
                        .enumerate()
                        .map(|(t, p)| p * f64::from(v[(t * 4 + head / 2) * 8 + c]))
                        .sum();
                }
                expected_scores.extend(scores);
                expected_probabilities.extend(probability);
            }
        }
        let build = |values: &[f32], traces: bool| {
            let mut b = CompositionBuilder::new(base.clone(), limits).unwrap();
            let import = |data: &[f32], dims: &[usize]| {
                Tensor::from_vec(
                    Shape::new(dims).unwrap(),
                    data.iter()
                        .map(|n| {
                            b.base_reference(
                                &base.constant(F32Bits::new(n.to_bits()).unwrap()).unwrap(),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            };
            let q = import(&q, &[3, 8, 8]);
            let k = import(&k, &[3, 4, 8]);
            let v = import(values, &[3, 4, 8]);
            let cos = import(&cos, &[3, 4]);
            let sin = import(&sin, &[3, 4]);
            let scale = import(&[scale], &[1]);
            let q = b.rope(&q.view(), &cos.view(), &sin.view()).unwrap();
            let k = b.rope(&k.view(), &cos.view(), &sin.view()).unwrap();
            let attention = b
                .attention(
                    &q.output.view(),
                    &k.output.view(),
                    &v.view(),
                    &scale.as_slice()[0],
                )
                .unwrap();
            let mut roots = attention.context.into_values();
            if traces {
                roots.extend(q.output.into_values());
                roots.extend(k.output.into_values());
                roots.extend(attention.scores.into_values());
                roots.extend(attention.probabilities.into_values());
            }
            b.finish(&roots).unwrap()
        };
        let (output, calls) = probe::observe(true, || build(&v, true));
        assert_eq!(calls, Default::default());
        let values = decode(&output, limits).unwrap();
        let mut expected = expected_context;
        expected.extend(qr);
        expected.extend(kr);
        expected.extend(expected_scores);
        expected.extend_from_slice(&expected_probabilities);
        close(&values.original, &expected);
        assert_eq!(values.original, values.planned);
        let later = decode(&build(&changed, false), limits).unwrap();
        assert_eq!(&values.original[..128], &later.original[..128]);
        let expected_change: Vec<_> = (0..64)
            .map(|i| expected_probabilities[(i / 8) * 6 + 5])
            .collect();
        let actual_change: Vec<_> = later.original[128..]
            .iter()
            .zip(&values.original[128..192])
            .map(|(b, a)| b - a)
            .collect();
        close(&actual_change, &expected_change);
        // Primera posición solo ve un token: cada par Q comparte exactamente su KV.
        close(
            &values.original[..64],
            &(0..64)
                .map(|i| f64::from(v[(i / 8 / 2) * 8 + i % 8]))
                .collect::<Vec<_>>(),
        );
    }
}
