//! Tensores de referencias, contiguos row-major. Las operaciones solo emiten
//! One/Magma; no hay buffers numéricos ni instrucciones tensoriales ejecutables.
use super::{
    BaseRef, CompositionBuilder, NativeError, NativeErrorKind, NativeLimits, NativeRef, Phase,
    PreparedBase, Resource, TemplateBuilder, TemplateRef,
};
use crate::synth::structural::{Builder, model};
use std::{error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TensorError {
    Rank {
        actual: usize,
    },
    EmptyDimension {
        axis: usize,
    },
    SizeOverflow,
    Length {
        expected: usize,
        actual: usize,
    },
    Dimension {
        axis: usize,
        expected: usize,
        actual: usize,
    },
    Coordinate {
        axis: usize,
        index: usize,
        bound: usize,
    },
}
impl fmt::Display for TensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tensor {self:?}")
    }
}
impl Error for TensorError {}

/// Hasta cuatro ejes contiguos; forma escalar `[]` tiene un elemento. Se rechazan
/// ejes vacíos y productos desbordados antes de reservar handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    dimensions: [usize; 4],
    rank: usize,
    elements: usize,
}
impl Shape {
    pub fn new(dimensions: &[usize]) -> Result<Self, TensorError> {
        if dimensions.len() > 4 {
            return Err(TensorError::Rank {
                actual: dimensions.len(),
            });
        }
        let mut result = Self {
            dimensions: [0; 4],
            rank: dimensions.len(),
            elements: 1,
        };
        for (axis, &dimension) in dimensions.iter().enumerate() {
            if dimension == 0 {
                return Err(TensorError::EmptyDimension { axis });
            }
            result.elements = result
                .elements
                .checked_mul(dimension)
                .ok_or(TensorError::SizeOverflow)?;
            result.dimensions[axis] = dimension;
        }
        Ok(result)
    }
    pub fn dimensions(&self) -> &[usize] {
        &self.dimensions[..self.rank]
    }
    pub fn elements(&self) -> usize {
        self.elements
    }
    pub fn index(&self, coordinates: &[usize]) -> Result<usize, TensorError> {
        if coordinates.len() != self.rank {
            return Err(TensorError::Rank {
                actual: coordinates.len(),
            });
        }
        let mut index = 0;
        for (axis, (&coordinate, &dimension)) in
            coordinates.iter().zip(self.dimensions()).enumerate()
        {
            if coordinate >= dimension {
                return Err(TensorError::Coordinate {
                    axis,
                    index: coordinate,
                    bound: dimension,
                });
            }
            index = index * dimension + coordinate; // Acotado por elements, ya comprobado.
        }
        Ok(index)
    }
}

#[derive(Debug)]
pub struct Tensor<R> {
    shape: Shape,
    values: Vec<R>,
}
impl<R> Tensor<R> {
    pub fn from_vec(shape: Shape, values: Vec<R>) -> Result<Self, TensorError> {
        check_length(shape.elements, values.len())?;
        Ok(Self { shape, values })
    }
    pub fn view(&self) -> TensorView<'_, R> {
        TensorView {
            shape: self.shape,
            values: &self.values,
        }
    }
    pub fn shape(&self) -> Shape {
        self.shape
    }
    pub fn as_slice(&self) -> &[R] {
        &self.values
    }
    pub fn into_values(self) -> Vec<R> {
        self.values
    }
    pub fn reshape(mut self, shape: Shape) -> Result<Self, TensorError> {
        check_length(self.shape.elements, shape.elements)?;
        self.shape = shape;
        Ok(self)
    }
    pub fn storage_bytes(&self) -> (usize, usize) {
        (
            size_of_val(self.values.as_slice()),
            self.values.capacity() * size_of::<R>(),
        )
    }
}

#[derive(Debug)]
pub struct TensorView<'a, R> {
    shape: Shape,
    values: &'a [R],
}
impl<'a, R> TensorView<'a, R> {
    pub fn new(shape: Shape, values: &'a [R]) -> Result<Self, TensorError> {
        check_length(shape.elements, values.len())?;
        Ok(Self { shape, values })
    }
    pub fn shape(&self) -> Shape {
        self.shape
    }
    pub fn as_slice(&self) -> &'a [R] {
        self.values
    }
    pub fn get(&self, coordinates: &[usize]) -> Result<&'a R, TensorError> {
        Ok(&self.values[self.shape.index(coordinates)?])
    }
    pub fn reshape(&self, shape: Shape) -> Result<Self, TensorError> {
        Self::new(shape, self.values)
    }
    pub fn row(&self, row: usize) -> Result<&'a [R], TensorError> {
        let [_, columns] = matrix(self.shape)?;
        let start = self.shape.index(&[row, 0])?;
        Ok(&self.values[start..start + columns])
    }
}

pub(crate) fn check_length(expected: usize, actual: usize) -> Result<(), TensorError> {
    if expected == actual {
        Ok(())
    } else {
        Err(TensorError::Length { expected, actual })
    }
}
pub(crate) fn matrix(shape: Shape) -> Result<[usize; 2], TensorError> {
    if let [rows, columns] = *shape.dimensions() {
        Ok([rows, columns])
    } else {
        Err(TensorError::Rank { actual: shape.rank })
    }
}

pub(crate) trait TensorBuilder: Builder<Error = NativeError> {
    fn phase(&self) -> Phase;
    fn limits(&self) -> NativeLimits;
    fn base(&self) -> &Arc<PreparedBase>;
    fn import_base(&self, reference: &BaseRef) -> Result<Self::Ref, NativeError>;
    fn validate_reference(&self, reference: &Self::Ref) -> Result<(), NativeError>;

    fn tensor_error(&self, error: TensorError) -> NativeError {
        NativeError {
            phase: self.phase(),
            kind: NativeErrorKind::Tensor(error),
        }
    }
    fn reserve<T>(&self, values: &mut Vec<T>, additional: usize) -> Result<(), NativeError> {
        self.limits()
            .reserve(self.phase(), Resource::Roots, values, additional)
    }
    fn validate_slice(&self, values: &[Self::Ref]) -> Result<(), NativeError> {
        for value in values {
            self.validate_reference(value)?;
        }
        Ok(())
    }
}

pub(crate) fn base_tensor<B: TensorBuilder>(
    builder: &B,
    index: usize,
) -> Result<Tensor<B::Ref>, NativeError> {
    let tensor = builder.base().tensors().get(index).ok_or(NativeError {
        phase: builder.phase(),
        kind: NativeErrorKind::InvalidReference { index },
    })?;
    let shape = Shape::new(tensor.shape()).map_err(|e| builder.tensor_error(e))?;
    let mut values = Vec::new();
    builder.reserve(&mut values, shape.elements)?;
    for coordinate in 0..shape.elements {
        let (_, reference) = builder
            .base()
            .tensor_scalar(index, coordinate)
            .expect("validated base tensor");
        values.push(builder.import_base(&reference)?);
    }
    Ok(Tensor { shape, values })
}

type Addition<B> = fn(
    &mut B,
    &<B as Builder>::Ref,
    &<B as Builder>::Ref,
) -> Result<<B as Builder>::Ref, NativeError>;

/// Reducción de pares adyacentes, trasladando el impar al nivel siguiente.
/// `add` selecciona suma firmada o positiva; nunca reordena por valores.
pub(crate) fn reduce<B: TensorBuilder>(
    builder: &mut B,
    mut values: Vec<B::Ref>,
    add: Addition<B>,
) -> Result<B::Ref, NativeError> {
    if values.is_empty() {
        return Err(builder.tensor_error(TensorError::EmptyDimension { axis: 0 }));
    }
    builder.validate_slice(&values)?;
    while values.len() > 1 {
        let length = values.len();
        for index in 0..length.div_ceil(2) {
            let first = 2 * index;
            values[index] = if first + 1 < length {
                add(builder, &values[first], &values[first + 1])?
            } else {
                values[first].clone()
            };
        }
        values.truncate(length.div_ceil(2));
    }
    Ok(values.pop().expect("nonempty reduction"))
}

pub(crate) fn balanced_sum<B: TensorBuilder>(
    builder: &mut B,
    input: &[B::Ref],
) -> Result<B::Ref, NativeError> {
    let mut values = Vec::new();
    builder.reserve(&mut values, input.len())?;
    values.extend_from_slice(input);
    reduce(builder, values, model::add)
}

pub(crate) fn dot<B: TensorBuilder>(
    builder: &mut B,
    left: &[B::Ref],
    right: &[B::Ref],
) -> Result<B::Ref, NativeError> {
    check_length(left.len(), right.len()).map_err(|e| builder.tensor_error(e))?;
    builder.validate_slice(left)?;
    builder.validate_slice(right)?;
    let mut products = Vec::new();
    builder.reserve(&mut products, left.len())?;
    for (a, b) in left.iter().zip(right) {
        products.push(model::mul(builder, a, b)?);
    }
    reduce(builder, products, model::add)
}

pub(crate) fn project<B: TensorBuilder>(
    builder: &mut B,
    input: &TensorView<'_, B::Ref>,
    weight: &TensorView<'_, B::Ref>,
) -> Result<Tensor<B::Ref>, NativeError> {
    let [positions, channels] = matrix(input.shape).map_err(|e| builder.tensor_error(e))?;
    let [outputs, width] = matrix(weight.shape).map_err(|e| builder.tensor_error(e))?;
    check_length(channels, width).map_err(|e| builder.tensor_error(e))?;
    let shape = Shape::new(&[positions, outputs]).map_err(|e| builder.tensor_error(e))?;
    builder.validate_slice(input.values)?;
    builder.validate_slice(weight.values)?;
    let mut values = Vec::new();
    builder.reserve(&mut values, shape.elements)?;
    for position in 0..positions {
        for output in 0..outputs {
            values.push(dot(
                builder,
                input.row(position).expect("bounded row"),
                weight.row(output).expect("bounded row"),
            )?);
        }
    }
    Ok(Tensor { shape, values })
}

pub(crate) fn residual<B: TensorBuilder>(
    builder: &mut B,
    left: &TensorView<'_, B::Ref>,
    right: &TensorView<'_, B::Ref>,
) -> Result<Tensor<B::Ref>, NativeError> {
    if left.shape.rank != right.shape.rank {
        return Err(builder.tensor_error(TensorError::Rank {
            actual: right.shape.rank,
        }));
    }
    for (axis, (&expected, &actual)) in left
        .shape
        .dimensions()
        .iter()
        .zip(right.shape.dimensions())
        .enumerate()
    {
        if expected != actual {
            return Err(builder.tensor_error(TensorError::Dimension {
                axis,
                expected,
                actual,
            }));
        }
    }
    builder.validate_slice(left.values)?;
    builder.validate_slice(right.values)?;
    let mut values = Vec::new();
    builder.reserve(&mut values, left.values.len())?;
    for (a, b) in left.values.iter().zip(right.values) {
        values.push(model::add(builder, a, b)?);
    }
    Ok(Tensor {
        shape: left.shape,
        values,
    })
}

// Fronteras públicas tipadas; la implementación estructural genérica queda interna.
macro_rules! tensor_methods {
    ($builder:ty, $reference:ty) => {
        impl $builder {
            pub fn base_tensor(&self, index: usize) -> Result<Tensor<$reference>, NativeError> {
                base_tensor(self, index)
            }
            pub fn balanced_sum(
                &mut self,
                values: &[$reference],
            ) -> Result<$reference, NativeError> {
                balanced_sum(self, values)
            }
            pub fn dot(
                &mut self,
                left: &[$reference],
                right: &[$reference],
            ) -> Result<$reference, NativeError> {
                dot(self, left, right)
            }
            pub fn project(
                &mut self,
                input: &TensorView<'_, $reference>,
                weight: &TensorView<'_, $reference>,
            ) -> Result<Tensor<$reference>, NativeError> {
                project(self, input, weight)
            }
            pub fn residual(
                &mut self,
                left: &TensorView<'_, $reference>,
                right: &TensorView<'_, $reference>,
            ) -> Result<Tensor<$reference>, NativeError> {
                residual(self, left, right)
            }
        }
    };
}
tensor_methods!(TemplateBuilder, TemplateRef);
tensor_methods!(CompositionBuilder, NativeRef);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::probe,
        model_io::F32Bits,
        native::{NativePrefix, PreparedModel, decode},
    };

    #[test]
    fn row_major_shapes_views_and_overflow_are_checked() {
        assert!(matches!(
            Shape::new(&[usize::MAX, 2]),
            Err(TensorError::SizeOverflow)
        ));
        assert!(Shape::new(&[1, 0]).is_err());
        assert!(Shape::new(&[1; 5]).is_err());
        let shape = Shape::new(&[2, 3, 4]).unwrap();
        assert_eq!(shape.index(&[1, 2, 3]), Ok(23));
        assert!(shape.index(&[0, 3, 0]).is_err());
        assert!(shape.index(&[1, 2]).is_err());
        let values: Vec<_> = (0..24).collect();
        let tensor = Tensor::from_vec(shape, values).unwrap();
        let view = tensor.view().reshape(Shape::new(&[6, 4]).unwrap()).unwrap();
        assert_eq!(view.row(5).unwrap(), &[20, 21, 22, 23]);
        assert!(view.row(6).is_err());
        assert!(view.reshape(Shape::new(&[23]).unwrap()).is_err());
        assert!(Tensor::from_vec(Shape::new(&[2]).unwrap(), vec![1]).is_err());
    }

    #[test]
    fn native_dots_projection_residuals_match_independent_oracles() {
        let limits = NativeLimits::default();
        let bits: Vec<_> = [
            0.0f32,
            1.0,
            2.0,
            3.0,
            4.0,
            5.0,
            6.0,
            -1.0,
            128.0,
            1.0 / 64.0,
            1.0 / 8.0,
            4096.0,
            512.0,
        ]
        .into_iter()
        .map(|n| F32Bits::new(n.to_bits()).unwrap())
        .collect();
        let base = Arc::new(PreparedBase::from_constants(&bits, limits).unwrap());
        let mut builder = CompositionBuilder::new(base.clone(), limits).unwrap();
        let constant = |n: f32| {
            builder
                .base_reference(&base.constant(F32Bits::new(n.to_bits()).unwrap()).unwrap())
                .unwrap()
        };
        let weights = Tensor::from_vec(
            Shape::new(&[2, 2]).unwrap(),
            [1.0, 2.0, 3.0, 4.0].map(constant).to_vec(),
        )
        .unwrap();
        let input = Tensor::from_vec(
            Shape::new(&[2, 2]).unwrap(),
            [5.0, 6.0, -1.0, 1.0].map(constant).to_vec(),
        )
        .unwrap();
        let (result, calls) = probe::observe(true, || {
            builder.project(&input.view(), &weights.view()).unwrap()
        });
        assert_eq!(calls, Default::default());
        let doubled = builder.residual(&result.view(), &result.view()).unwrap();
        let left: Vec<_> = [1.0f32, -1.0, 1.0]
            .into_iter()
            .map(|n| {
                builder
                    .base_reference(&base.constant(F32Bits::new(n.to_bits()).unwrap()).unwrap())
                    .unwrap()
            })
            .collect();
        let dot = builder.dot(&left, &left).unwrap();
        let odd_sum = builder.balanced_sum(&left).unwrap();
        let mut roots = result.into_values();
        roots.extend(doubled.into_values());
        roots.extend([dot, odd_sum]);
        let output = builder.finish(&roots).unwrap();
        let values = decode(&output, limits).unwrap();
        for (&actual, expected) in values
            .original
            .iter()
            .zip([17.0, 39.0, 1.0, 1.0, 34.0, 78.0, 2.0, 2.0, 3.0, 1.0])
        {
            assert!((actual - expected).abs() <= 1e-6 + 1e-5 * expected.abs());
        }
        assert_eq!(values.original, values.planned);
    }

    #[test]
    fn template_aliases_and_foreign_handles_are_preserved_through_projection() {
        let limits = NativeLimits::default();
        let base = Arc::new(PreparedBase::vocabulary_fixture(8, 2));
        let mut builder = TemplateBuilder::new(base.clone(), 2, limits).unwrap();
        let embedding = builder.base_tensor(0).unwrap();
        let output = builder.base_tensor(1).unwrap();
        assert_eq!(embedding.as_slice(), output.as_slice());
        let other = TemplateBuilder::new(base.clone(), 2, limits).unwrap();
        let foreign = other.port(0).unwrap();
        assert!(builder.balanced_sum(&[foreign]).is_err()); // También longitud uno.
        let a = builder.port(0).unwrap();
        let b = builder.port(1).unwrap();
        let before = builder.instruction_count();
        assert!(builder.dot(std::slice::from_ref(&a), &[]).is_err());
        assert_eq!(builder.instruction_count(), before);
        let input = Tensor::from_vec(Shape::new(&[1, 2]).unwrap(), vec![a, b]).unwrap();
        let projected = builder.project(&input.view(), &output.view()).unwrap();
        let doubled = builder
            .residual(&projected.view(), &projected.view())
            .unwrap();
        let template = builder.finish(doubled.as_slice()).unwrap();
        let model = PreparedModel::from_template(template, 1).unwrap();
        let prefix = NativePrefix::encode(base, &[1], &[0], 1, limits).unwrap();
        let values = decode(&model.compose(&prefix, limits).unwrap(), limits).unwrap();
        for (&actual, expected) in values
            .original
            .iter()
            .zip([0.0f64, 8.0, 4.0, 0.0, 8.0, 4.0, 0.0, 8.0])
        {
            assert!((actual - expected).abs() <= 1e-6 + 1e-5 * expected.abs());
        }
    }
}
