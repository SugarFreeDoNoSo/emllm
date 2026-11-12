//! Validación estructural y compilación iterativa de las raíces seleccionadas.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, NodeId};

/// Forma cerrada de los nodos que expone la inspección del programa.
pub use crate::expr::Node;

/// Snapshot cerrado cuyos hijos preceden a sus padres en `nodes()`.
///
/// Posee sus nodos y raíces: ampliar o liberar la arena no cambia el programa.
/// Solo [`compile`] puede producirlo desde una construcción. Clonarlo copia un
/// snapshot ya válido; no concede acceso mutable a sus campos.
///
/// ```
/// use emllm::{eval::evaluate, expr::{Arena, Node}, program::compile};
/// let mut arena = Arena::new(3);
/// let one = arena.one().unwrap();
/// let root = arena.magma(&one, &one).unwrap();
/// let program = compile(&arena, std::slice::from_ref(&root), 2).unwrap();
/// arena.magma(&root, &one).unwrap();
/// drop(arena);
/// assert_eq!(program.nodes(), &[Node::One, Node::Magma(0, 0)]);
/// assert_eq!(program.roots(), &[1]);
/// assert!((evaluate(&program, 2).unwrap()[0] - std::f64::consts::E).abs() <= 1e-12);
/// ```
///
/// Los campos privados impiden fabricar otro programa saltándose la validación:
///
/// ```compile_fail
/// use emllm::{expr::{Arena, Node}, program::{Program, compile}};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let program = compile(&arena, &[one], 1).unwrap();
/// let invalid = Program { nodes: vec![Node::Magma(99, 99)], ..program };
/// ```
///
/// Ni los nodos ni las raíces ofrecen acceso mutable:
///
/// ```compile_fail
/// use emllm::{expr::{Arena, Node}, program::compile};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let mut program = compile(&arena, &[one], 1).unwrap();
/// program.nodes()[0] = Node::Magma(99, 99);
/// ```
///
/// ```compile_fail
/// use emllm::{expr::Arena, program::compile};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let mut program = compile(&arena, &[one], 1).unwrap();
/// program.roots()[0] = 99;
/// ```
#[derive(Clone, Debug)]
pub struct Program {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    stats: ProgramStats,
}

/// Estadísticas de la clausura alcanzable, calculadas al compilar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramStats {
    /// V: nodos únicos.
    pub nodes: usize,
    /// E: posiciones de operando, incluso si repiten el mismo hijo.
    pub edges: usize,
    /// R: posiciones de raíz, incluidos duplicados.
    pub roots: usize,
    /// D: profundidad máxima en enlaces; One tiene profundidad cero.
    pub depth: usize,
}

impl Program {
    /// Consulta constante de estadísticas, sin evaluación ni recorrido adicional.
    pub fn stats(&self) -> &ProgramStats {
        &self.stats
    }

    /// Vista inmutable en orden de evaluación. Sus posiciones identifican nodos
    /// únicamente dentro de este snapshot, incluidos los operandos de Magma.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Resuelve una posición de raíz, operando o diagnóstico en este programa.
    /// No acepta un handle de construcción [`NodeId`]; sus índices pueden haber
    /// sido remapeados. Un índice fuera del snapshot devuelve `None`.
    pub fn node(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }

    /// Posiciones del snapshot, en el orden solicitado y conservando duplicados.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }
}

/// Error estructural: no se entrega un [`Program`] parcial.
/// Los índices de referencia/ciclo pertenecen al origen, no a un snapshot.
/// `Roots` conserva la causa de construcción; `NodeLimit` no identifica nodo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    Roots(BuildError),
    InvalidReference { index: usize },
    Cycle { index: usize },
    NodeLimit { limit: usize },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Roots(error) => write!(f, "invalid roots: {error}"),
            Self::InvalidReference { index } => write!(f, "invalid graph reference {index}"),
            Self::Cycle { index } => write!(f, "cycle at node {index}"),
            Self::NodeLimit { limit } => write!(f, "compilation node budget {limit} exhausted"),
        }
    }
}

impl Error for CompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Roots(error) => Some(error),
            _ => None,
        }
    }
}

/// Compila únicamente nodos alcanzables; no realiza evaluación numérica.
/// Visita primero el operando izquierdo y conserva el orden y los duplicados de
/// las raíces. Una misma estructura y lista de raíces producen el mismo orden.
/// El límite cuenta nodos únicos alcanzables, no posiciones de raíz.
pub fn compile(arena: &Arena, roots: &[NodeId], max_nodes: usize) -> Result<Program, CompileError> {
    arena.validate_roots(roots).map_err(CompileError::Roots)?;
    let roots: Vec<_> = roots.iter().map(NodeId::index).collect();
    compile_nodes(arena.nodes(), &roots, max_nodes)
}

fn compile_nodes(
    source: &[Node],
    roots: &[usize],
    max_nodes: usize,
) -> Result<Program, CompileError> {
    let mut states = HashMap::new();
    let mut remapped = HashMap::new();
    let mut nodes = Vec::new();
    let mut depths: Vec<usize> = Vec::new();
    let mut edges = 0;
    let mut stack = Vec::new();

    for &root in roots {
        stack.push((root, false));
        while let Some((index, exiting)) = stack.pop() {
            let node = *source
                .get(index)
                .ok_or(CompileError::InvalidReference { index })?;
            if exiting {
                let (compiled, depth) = match node {
                    Node::One => (Node::One, 0),
                    Node::Magma(left, right) => {
                        let left = remapped[&left];
                        let right = remapped[&right];
                        edges += 2;
                        (Node::Magma(left, right), 1 + depths[left].max(depths[right]))
                    }
                };
                remapped.insert(index, nodes.len());
                nodes.push(compiled);
                depths.push(depth);
                states.insert(index, 2);
                continue;
            }
            match states.get(&index) {
                Some(2) => continue,
                Some(1) => return Err(CompileError::Cycle { index }),
                _ => {}
            }
            if states.len() >= max_nodes {
                return Err(CompileError::NodeLimit { limit: max_nodes });
            }
            states.insert(index, 1);
            stack.push((index, true));
            if let Node::Magma(left, right) = node {
                stack.push((right, false));
                stack.push((left, false));
            }
        }
    }

    Ok(Program {
        stats: ProgramStats {
            nodes: nodes.len(),
            edges,
            roots: roots.len(),
            depth: depths.into_iter().max().unwrap_or(0),
        },
        nodes,
        roots: roots.iter().map(|root| remapped[root]).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_graphs() {
        assert!(matches!(
            compile_nodes(&[Node::Magma(0, 0)], &[0], 10),
            Err(CompileError::Cycle { index: 0 })
        ));
        assert!(matches!(
            compile_nodes(&[Node::Magma(1, 2), Node::One], &[0], 10),
            Err(CompileError::InvalidReference { index: 2 })
        ));
        assert!(matches!(
            compile_nodes(&[Node::Magma(1, 1), Node::Magma(0, 0)], &[0], 10),
            Err(CompileError::Cycle { .. })
        ));
    }

    #[test]
    fn compiles_deep_graphs_without_recursion() {
        let mut arena = Arena::new(20_001);
        let one = arena.one().unwrap();
        let mut root = one.clone();
        for _ in 0..20_000 {
            root = arena.magma(&root, &one).unwrap();
        }
        let program = compile(&arena, &[root], 20_001).unwrap();
        assert_eq!(program.nodes().len(), 20_001);
        for (index, node) in program.nodes().iter().enumerate() {
            if let Node::Magma(left, right) = node {
                assert!(*left < index && *right < index);
            }
        }
    }

    #[test]
    fn compilation_budget_counts_reachable_nodes() {
        let mut arena = Arena::new(3);
        let one = arena.one().unwrap();
        let e = arena.magma(&one, &one).unwrap();
        arena.magma(&e, &one).unwrap();
        assert_eq!(compile(&arena, &[one], 1).unwrap().nodes().len(), 1);
        assert!(matches!(
            compile(&arena, &[e], 1),
            Err(CompileError::NodeLimit { limit: 1 })
        ));
        assert!(matches!(
            compile(&arena, &[], 3),
            Err(CompileError::Roots(BuildError::EmptyRoots))
        ));
    }
}
