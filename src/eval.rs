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

use crate::program::Program;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalErrorKind {
    /// El segundo operando es cero o negativo.
    LogDomain,
    /// La exponencial de un operando finito no es finita.
    ExponentialOverflow,
    /// Un operando o resultado no es finito.
    NonFinite,
    /// Rechazo previo a ejecutar nodos.
    NodeLimit { limit: usize },
}

/// `node` identifica un nodo lógico del programa, no una fila ni una posición
/// de la arena original. Un Magma de índice i está en la fila i-1.
/// Es `Some` para fallos numéricos y `None` para el rechazo previo de presupuesto.
/// Resuélvase con [`Program::node`] sobre el programa de esta evaluación.
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

/// Tamaños de los dos vectores de una ejecución satisfactoria, capturados antes
/// de liberar los temporales. Capacidad incluye elementos ocupados y sobrantes.
/// Excluye asignador, otros locales, registros de diagnóstico y RSS del proceso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationStorage {
    pub values: usize,
    pub value_payload_bytes: usize,
    pub value_capacity_bytes: usize,
    pub output_payload_bytes: usize,
    pub output_capacity_bytes: usize,
    pub buffer_metadata_bytes: usize,
}

impl EvaluationStorage {
    pub(crate) fn capture(values: &Vec<f64>, outputs: &Vec<f64>) -> Self {
        Self {
            values: values.len(),
            value_payload_bytes: std::mem::size_of_val(values.as_slice()),
            value_capacity_bytes: values.capacity() * std::mem::size_of::<f64>(),
            output_payload_bytes: std::mem::size_of_val(outputs.as_slice()),
            output_capacity_bytes: outputs.capacity() * std::mem::size_of::<f64>(),
            buffer_metadata_bytes: 2 * std::mem::size_of::<Vec<f64>>(),
        }
    }
}

/// Evalúa cada nodo compartido una sola vez, en el orden ya compilado.
/// Solo acepta un [`Program`] validado y su presupuesto explícito; no recibe
/// raíces adicionales. La respuesta contiene todas las salidas o un error.
/// Cada llamada posee su buffer de valores y recibe su propio límite; no guarda
/// cachés, configuración ambiental ni resultados numéricos en el programa.
/// One consume un nodo aunque no ocupe fila. Tras comprobar el presupuesto se
/// inicializa su valor 1.0 y se recorren pares; la fila k ejecuta el nodo k+1.
///
/// # Errors
///
/// Devuelve [`EvalErrorKind::NodeLimit`] antes de ejecutar si el presupuesto es
/// insuficiente. La primera operación inválida devuelve dominio, desbordamiento
/// o no finitud con su índice; no devuelve salidas parciales como éxito.
///
/// ```
/// use emllm::{eval::evaluate, expr::Arena, program::compile};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let program = compile(&arena, &[one], 1).unwrap();
/// assert_eq!(evaluate(&program, 1).unwrap(), vec![1.0]);
/// ```
///
/// Una arena no acredita un programa validado:
///
/// ```compile_fail
/// use emllm::{eval::evaluate, expr::Arena};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// evaluate(&arena, 1);
/// ```
///
/// Tampoco una referencia de construcción:
///
/// ```compile_fail
/// use emllm::{eval::evaluate, expr::Arena};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// evaluate(&one, 1);
/// ```
///
/// Ni una colección de nodos inspeccionables, incluso si parece válida:
///
/// ```compile_fail
/// use emllm::{eval::evaluate, expr::Node};
/// let nodes = vec![Node::One];
/// evaluate(&nodes, 1);
/// ```
pub fn evaluate(program: &Program, max_nodes: usize) -> Result<Vec<f64>, EvalError> {
    evaluate_with_storage(program, max_nodes).map(|(outputs, _)| outputs)
}

/// Mismo recorrido y errores que [`evaluate`], con contabilidad de los buffers
/// realmente asignados. No reutiliza resultados ni retiene el buffer de valores.
pub fn evaluate_with_storage(
    program: &Program,
    max_nodes: usize,
) -> Result<(Vec<f64>, EvaluationStorage), EvalError> {
    if program.node_count() > max_nodes {
        return Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: max_nodes },
            node: None,
        });
    }
    let mut values: Vec<f64> = Vec::with_capacity(program.node_count());
    values.push(1.0);
    for (row, &[left, right]) in program.pairs().iter().enumerate() {
        let value = magma_value(values[left], values[right], row + 1)?;
        values.push(value);
    }
    let outputs = program.roots().iter().map(|&root| values[root]).collect();
    let storage = EvaluationStorage::capture(&values, &outputs);
    Ok((outputs, storage))
}

pub(crate) fn magma_value(left: f64, right: f64, index: usize) -> Result<f64, EvalError> {
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
