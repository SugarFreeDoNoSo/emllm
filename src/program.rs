//! Validación estructural y compilación iterativa de las raíces seleccionadas.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, Node, NodeId};

/// Snapshot cerrado cuyos hijos preceden a sus padres en `nodes()`.
#[derive(Clone, Debug)]
pub struct Program {
    nodes: Vec<Node>,
    roots: Vec<usize>,
}

impl Program {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Posiciones del snapshot, en el orden solicitado y conservando duplicados.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }
}

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
    let mut stack = Vec::new();

    for &root in roots {
        stack.push((root, false));
        while let Some((index, exiting)) = stack.pop() {
            let node = *source
                .get(index)
                .ok_or(CompileError::InvalidReference { index })?;
            if exiting {
                let compiled = match node {
                    Node::One => Node::One,
                    Node::Magma(left, right) => Node::Magma(remapped[&left], remapped[&right]),
                };
                remapped.insert(index, nodes.len());
                nodes.push(compiled);
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
