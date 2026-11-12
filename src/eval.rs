//! Evaluación numérica de programas One/Magma.

use crate::expr::Node;
use crate::program::Program;

/// Evalúa cada nodo compartido una sola vez, en el orden ya compilado.
pub fn evaluate(program: &Program) -> Vec<f64> {
    let mut values: Vec<f64> = Vec::with_capacity(program.nodes().len());
    for node in program.nodes() {
        let value = match *node {
            Node::One => 1.0,
            Node::Magma(left, right) => values[left].exp() - values[right].ln(),
        };
        values.push(value);
    }
    program.roots().iter().map(|&root| values[root]).collect()
}
