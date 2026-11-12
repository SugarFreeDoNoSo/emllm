//! Representación estructural de expresiones One/Magma.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Las únicas formas ejecutables; las posiciones de hijos son locales a la arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    One,
    Magma(usize, usize),
}

/// Referencia opaca que conserva la identidad de su arena de origen.
///
/// El token compartido permanece vivo mientras exista cualquier referencia;
/// liberar una arena no permite reutilizar su identidad para esos handles.
/// La posición local por sí sola no acredita procedencia.
///
/// ```
/// use emllm::expr::{Arena, NodeId};
/// let mut arena = Arena::new(1);
/// let id: NodeId = arena.one().unwrap();
/// assert_eq!(id, id.clone());
/// assert_eq!(id.index(), 0);
/// ```
///
/// Un consumidor no puede fabricar los campos privados:
///
/// ```compile_fail
/// use emllm::expr::NodeId;
/// use std::sync::Arc;
/// let id = NodeId { arena: Arc::new(()), index: 0 };
/// ```
#[derive(Clone, Debug)]
pub struct NodeId {
    arena: Arc<()>,
    index: usize,
}

impl NodeId {
    /// Posición para inspección y diagnóstico, no un constructor de referencias.
    pub fn index(&self) -> usize {
        self.index
    }
}

impl PartialEq for NodeId {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.arena, &other.arena)
    }
}

impl Eq for NodeId {}

/// Fallos de construcción o selección de raíces, distinguibles sin leer texto.
/// `InvalidReference` identifica una posición de arena; los demás no inventan
/// un nodo. Un error no entrega un handle nuevo como éxito.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    ForeignReference,
    InvalidReference { index: usize },
    EmptyRoots,
    NodeLimit { limit: usize },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignReference => write!(f, "reference belongs to another arena"),
            Self::InvalidReference { index } => write!(f, "invalid node reference {index}"),
            Self::EmptyRoots => write!(f, "at least one root is required"),
            Self::NodeLimit { limit } => write!(f, "construction node budget {limit} exhausted"),
        }
    }
}

impl Error for BuildError {}

/// Arena append-only: un nodo solo puede referirse a nodos ya existentes.
/// Las posiciones nunca se reutilizan. No implementa `Clone`: copiar su identidad
/// a un constructor independiente rompería la procedencia de sus referencias.
/// Sus vistas son inmutables; la síntesis puede dejar nodos internos válidos al
/// agotar presupuesto, pero no devuelve la raíz fallida como éxito.
#[derive(Debug)]
pub struct Arena {
    identity: Arc<()>,
    nodes: Vec<Node>,
    interned: HashMap<Node, usize>,
    max_nodes: usize,
}

impl Arena {
    /// El presupuesto cuenta nodos únicos de toda la arena, no sus usos.
    pub fn new(max_nodes: usize) -> Self {
        Self {
            identity: Arc::new(()),
            nodes: Vec::new(),
            interned: HashMap::new(),
            max_nodes,
        }
    }

    /// Nodos lógicos únicos, incluidos One y los Magma construidos.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Inspección por copia en orden de construcción, sin asignar una colección.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Node> + '_ {
        self.nodes.iter().copied()
    }

    /// Copia de un nodo local; modificarla no altera la arena.
    /// Devuelve `None` para cualquier índice fuera de rango.
    pub fn node(&self, index: usize) -> Option<Node> {
        self.nodes.get(index).copied()
    }

    pub fn node_limit(&self) -> usize {
        self.max_nodes
    }

    pub fn one(&mut self) -> Result<NodeId, BuildError> {
        self.intern(Node::One)
    }

    pub fn magma(&mut self, left: &NodeId, right: &NodeId) -> Result<NodeId, BuildError> {
        let left = self.validate_node(left)?;
        let right = self.validate_node(right)?;
        self.intern(Node::Magma(left, right))
    }

    /// Comprueba presencia y procedencia; no evalúa ni produce un programa.
    /// Para obtener un snapshot ejecutable se usa [`crate::program::compile`].
    pub fn validate_roots(&self, roots: &[NodeId]) -> Result<(), BuildError> {
        if roots.is_empty() {
            return Err(BuildError::EmptyRoots);
        }
        for root in roots {
            self.validate_node(root)?;
        }
        Ok(())
    }

    pub(crate) fn validate_node(&self, id: &NodeId) -> Result<usize, BuildError> {
        if !Arc::ptr_eq(&self.identity, &id.arena) {
            return Err(BuildError::ForeignReference);
        }
        if id.index >= self.nodes.len() {
            return Err(BuildError::InvalidReference { index: id.index });
        }
        Ok(id.index)
    }

    fn intern(&mut self, node: Node) -> Result<NodeId, BuildError> {
        let index = if let Some(&index) = self.interned.get(&node) {
            index
        } else {
            if self.nodes.len() >= self.max_nodes {
                return Err(BuildError::NodeLimit {
                    limit: self.max_nodes,
                });
            }
            let index = self.nodes.len();
            self.nodes.push(node);
            self.interned.insert(node, index);
            index
        };
        Ok(NodeId {
            arena: self.identity.clone(),
            index,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_structure_and_preserves_operand_order() {
        let mut arena = Arena::new(10);
        let one = arena.one().unwrap();
        assert_eq!(one, arena.one().unwrap());
        let e = arena.magma(&one, &one).unwrap();
        assert_eq!(e, arena.magma(&one, &one).unwrap());
        let left = arena.magma(&one, &e).unwrap();
        let right = arena.magma(&e, &one).unwrap();
        assert_ne!(left, right);
        assert_eq!(arena.nodes.len(), 4);
    }

    #[test]
    fn rejects_foreign_operands_and_roots_even_at_the_same_index() {
        let mut first = Arena::new(10);
        let mut second = Arena::new(10);
        let a = first.one().unwrap();
        let b = second.one().unwrap();
        assert_eq!(a.index(), b.index());
        assert_ne!(a, b);
        assert_eq!(first.magma(&a, &b), Err(BuildError::ForeignReference));
        assert_eq!(first.magma(&b, &a), Err(BuildError::ForeignReference));
        assert_eq!(
            first.validate_roots(&[b]),
            Err(BuildError::ForeignReference)
        );
        assert_eq!(first.nodes.len(), 1);
    }

    #[test]
    fn future_references_cannot_introduce_cycles() {
        let mut arena = Arena::new(101);
        let one = arena.one().unwrap();
        let future = NodeId {
            arena: arena.identity.clone(),
            index: 1,
        };
        assert_eq!(
            arena.magma(&one, &future),
            Err(BuildError::InvalidReference { index: 1 })
        );
        assert_eq!(arena.nodes.len(), 1);
        let mut previous = one;
        for _ in 0..100 {
            let next = arena.magma(&previous, &previous).unwrap();
            assert!(previous.index() < next.index());
            previous = next;
        }
        assert!(arena.validate_roots(&[previous]).is_ok());
        assert_eq!(arena.validate_roots(&[]), Err(BuildError::EmptyRoots));
    }

    #[test]
    fn enforces_budget_without_charging_reused_nodes() {
        let mut empty = Arena::new(0);
        assert_eq!(empty.one(), Err(BuildError::NodeLimit { limit: 0 }));
        assert_eq!(empty.node_count(), 0);

        let mut arena = Arena::new(2);
        let one = arena.one().unwrap();
        let e = arena.magma(&one, &one).unwrap();
        assert_eq!(
            arena.magma(&one, &e),
            Err(BuildError::NodeLimit { limit: 2 })
        );
        assert_eq!(arena.one().unwrap(), one);
        assert_eq!(arena.magma(&one, &one).unwrap(), e);
        assert!(arena.nodes().eq([Node::One, Node::Magma(0, 0)]));
    }

    #[test]
    fn inspection_contains_only_closed_acyclic_nodes() {
        let mut arena = Arena::new(1000);
        let one = arena.one().unwrap();
        let mut previous = one.clone();
        for _ in 0..999 {
            previous = arena.magma(&previous, &one).unwrap();
        }
        for (index, node) in arena.nodes().enumerate() {
            match node {
                Node::One => assert_eq!(index, 0),
                Node::Magma(left, right) => {
                    assert!(left < index);
                    assert!(right < index);
                }
            }
        }
        assert_eq!(arena.node_count(), arena.node_limit());
    }
}
