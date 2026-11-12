//! Evaluación numérica de programas One/Magma.
//!
//! # Precisión y fallos
//!
//! Cada Magma ejecuta `exp(a) - ln(b)` en `f64`. La precisión de
//! [`f64::exp`] y [`f64::ln`] no está especificada y puede variar por plataforma,
//! versión o llamada. Las tolerancias de las fixtures son criterios comprobados
//! en esos casos, no una garantía global ni una promesa de igualdad bit a bit.
//!
//! La resta puede sufrir cancelación; por ejemplo, la expresión algebraica de
//! cero conserva su resultado calculado y nunca se reemplaza por un cero literal.
//! La construcción y compilación no ejecutan operaciones trascendentes.
//!
//! El subdesbordamiento de `exp` puede producir un subnormal o cero. Se conserva
//! ese valor finito. Si se utiliza posteriormente como argumento no positivo de
//! un logaritmo, esa operación devuelve un error de dominio. Los valores positivos
//! pequeños se aceptan tal cual, sin clamps, epsilons ni correcciones de signo.
//!
//! La primera operación numérica inválida termina la evaluación con un error;
//! no se entrega un vector parcial como éxito. Los identificadores del error son
//! locales al programa compilado. El presupuesto cuenta sus nodos alcanzables,
//! no tiempo de CPU ni bytes asignados.

use std::error::Error;
use std::fmt;

use crate::expr::Node;
use crate::program::Program;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalErrorKind {
    LogDomain,
    ExponentialOverflow,
    NonFinite,
    NodeLimit { limit: usize },
}

/// `node` identifica una posición de `Program::nodes()`, no de la arena original.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    pub kind: EvalErrorKind,
    pub node: Option<usize>,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evaluation {:?} at node {:?}", self.kind, self.node)
    }
}

impl Error for EvalError {}

/// Evalúa cada nodo compartido una sola vez, en el orden ya compilado.
pub fn evaluate(program: &Program, max_nodes: usize) -> Result<Vec<f64>, EvalError> {
    if program.nodes().len() > max_nodes {
        return Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: max_nodes },
            node: None,
        });
    }
    let mut values: Vec<f64> = Vec::with_capacity(program.nodes().len());
    for (index, node) in program.nodes().iter().enumerate() {
        let value = match *node {
            Node::One => 1.0,
            Node::Magma(left, right) => magma_value(values[left], values[right], index)?,
        };
        values.push(value);
    }
    Ok(program.roots().iter().map(|&root| values[root]).collect())
}

fn magma_value(left: f64, right: f64, index: usize) -> Result<f64, EvalError> {
    let error = |kind| EvalError {
        kind,
        node: Some(index),
    };
    if !left.is_finite() || !right.is_finite() {
        return Err(error(EvalErrorKind::NonFinite));
    }
    if right <= 0.0 {
        return Err(error(EvalErrorKind::LogDomain));
    }
    let exponential = left.exp();
    if !exponential.is_finite() {
        return Err(error(EvalErrorKind::ExponentialOverflow));
    }
    let result = exponential - right.ln();
    if !result.is_finite() {
        return Err(error(EvalErrorKind::NonFinite));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonfinite_operands_at_the_numeric_boundary() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (left, right) in [(invalid, 1.0), (1.0, invalid)] {
                assert_eq!(
                    magma_value(left, right, 7),
                    Err(EvalError {
                        kind: EvalErrorKind::NonFinite,
                        node: Some(7),
                    })
                );
            }
        }
    }

    #[test]
    fn rejects_zero_and_negative_log_arguments() {
        for right in [0.0, -0.0, -1.0] {
            assert_eq!(
                magma_value(1.0, right, 3),
                Err(EvalError {
                    kind: EvalErrorKind::LogDomain,
                    node: Some(3),
                })
            );
        }
    }

    #[test]
    fn preserves_underflow_and_accepts_positive_subnormal_arguments() {
        assert_eq!(magma_value(-1000.0, 1.0, 0).unwrap(), 0.0);
        let tiny = f64::from_bits(1);
        let value = magma_value(0.0, tiny, 0).unwrap();
        assert!((value - (1.0 - tiny.ln())).abs() <= 1e-12);
        assert!(value > 700.0);
    }
}
