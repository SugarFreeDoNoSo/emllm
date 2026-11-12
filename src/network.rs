//! Composición simbólica de una red neuronal pequeña.
//!
//! La red tiene dos entradas, dos neuronas ocultas con activación exp(-z*z)
//! y una salida afín. Cada invocación construye una nueva expresión cerrada;
//! ningún parámetro o entrada queda enlazado como valor numérico mutable.

use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, NodeId};
use crate::synth::{self, Rational};

#[derive(Clone, Debug)]
pub struct Network221 {
    hidden_weights: [[Rational; 2]; 2],
    hidden_biases: [Rational; 2],
    output_weights: [Rational; 2],
    output_bias: Rational,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    InputDimension { expected: usize, actual: usize },
    Build(BuildError),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputDimension { expected, actual } => {
                write!(f, "expected {expected} inputs, received {actual}")
            }
            Self::Build(error) => write!(f, "network construction failed: {error}"),
        }
    }
}

impl Error for NetworkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Build(error) => Some(error),
            _ => None,
        }
    }
}

impl From<BuildError> for NetworkError {
    fn from(error: BuildError) -> Self {
        Self::Build(error)
    }
}

impl Network221 {
    /// Las dimensiones de parámetros quedan fijadas por los tipos de los arrays.
    pub fn new(
        hidden_weights: [[Rational; 2]; 2],
        hidden_biases: [Rational; 2],
        output_weights: [Rational; 2],
        output_bias: Rational,
    ) -> Self {
        Self {
            hidden_weights,
            hidden_biases,
            output_weights,
            output_bias,
        }
    }

    /// Parámetros racionales del caso de aceptación documentado.
    pub fn fixture() -> Self {
        let zero = Rational::new(0, 1).expect("zero is in the rational domain");
        let half = Rational::new(1, 2).expect("one half is in the rational domain");
        let negative = Rational::new(-1, 2).expect("minus one half is in the rational domain");
        Self::new(
            [[half, negative], [zero, half]],
            [zero, negative],
            [half, negative],
            zero,
        )
    }

    /// Completa la expresión de salida sin evaluar ninguna capa.
    pub fn build(&self, arena: &mut Arena, inputs: &[Rational]) -> Result<NodeId, NetworkError> {
        if inputs.len() != 2 {
            return Err(NetworkError::InputDimension {
                expected: 2,
                actual: inputs.len(),
            });
        }
        let inputs = [
            synth::rational(arena, inputs[0])?,
            synth::rational(arena, inputs[1])?,
        ];
        let hidden = [
            self.hidden_neuron(arena, 0, &inputs)?,
            self.hidden_neuron(arena, 1, &inputs)?,
        ];
        Ok(affine(
            arena,
            &self.output_weights,
            self.output_bias,
            &hidden,
        )?)
    }

    fn hidden_neuron(
        &self,
        arena: &mut Arena,
        index: usize,
        inputs: &[NodeId; 2],
    ) -> Result<NodeId, BuildError> {
        let z = affine(
            arena,
            &self.hidden_weights[index],
            self.hidden_biases[index],
            inputs,
        )?;
        let square = synth::mul(arena, &z, &z)?;
        let negative = synth::neg(arena, &square)?;
        synth::exp(arena, &negative)
    }
}

fn affine(
    arena: &mut Arena,
    weights: &[Rational; 2],
    bias: Rational,
    inputs: &[NodeId; 2],
) -> Result<NodeId, BuildError> {
    let mut result = synth::rational(arena, bias)?;
    for (&weight, input) in weights.iter().zip(inputs) {
        let weight = synth::rational(arena, weight)?;
        let product = synth::mul(arena, &weight, input)?;
        result = synth::add(arena, &result, &product)?;
    }
    Ok(result)
}
