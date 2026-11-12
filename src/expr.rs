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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    ForeignReference,
    InvalidReference { index: usize },
    EmptyRoots,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignReference => write!(f, "reference belongs to another arena"),
            Self::InvalidReference { index } => write!(f, "invalid node reference {index}"),
            Self::EmptyRoots => write!(f, "at least one root is required"),
        }
    }
}

impl Error for BuildError {}

/// Arena append-only: un nodo solo puede referirse a nodos ya existentes.
#[derive(Debug)]
pub struct Arena {
    identity: Arc<()>,
    nodes: Vec<Node>,
    interned: HashMap<Node, usize>,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    pub fn new() -> Self {
        Self {
            identity: Arc::new(()),
            nodes: Vec::new(),
            interned: HashMap::new(),
        }
    }

    pub fn one(&mut self) -> Result<NodeId, BuildError> {
        self.intern(Node::One)
    }

    pub fn magma(&mut self, left: &NodeId, right: &NodeId) -> Result<NodeId, BuildError> {
        let left = self.validate_node(left)?;
        let right = self.validate_node(right)?;
        self.intern(Node::Magma(left, right))
    }

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
        let mut arena = Arena::new();
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
        let mut first = Arena::new();
        let mut second = Arena::new();
        let a = first.one().unwrap();
        let b = second.one().unwrap();
        assert_eq!(a.index(), b.index());
        assert_ne!(a, b);
        assert_eq!(first.magma(&a, &b), Err(BuildError::ForeignReference));
        assert_eq!(first.magma(&b, &a), Err(BuildError::ForeignReference));
        assert_eq!(first.validate_roots(&[b]), Err(BuildError::ForeignReference));
        assert_eq!(first.nodes.len(), 1);
    }

    #[test]
    fn future_references_cannot_introduce_cycles() {
        let mut arena = Arena::new();
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
}
