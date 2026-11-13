//! Representación estructural de expresiones One/Magma.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// Vista lógica de las únicas formas ejecutables; los hijos son índices locales
/// a la arena o al programa que produjo la copia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    One,
    Magma(usize, usize),
}

/// Almacenamiento del DAG: One ocupa el índice lógico 0 sin fila.
/// La fila k representa el nodo k+1. Los valores válidos solo añaden hijos
/// existentes; la marca distingue una arena vacía de One sin filas.
#[derive(Clone, Debug, Default)]
pub(crate) struct PairTable {
    pairs: Vec<[usize; 2]>,
    has_one: bool,
}

impl PairTable {
    /// Solo para fixtures internas de validación; no existe en la biblioteca
    /// de producción y puede contener referencias inexistentes o ciclos.
    #[cfg(test)]
    pub(crate) fn from_test_pairs(pairs: Vec<[usize; 2]>) -> Self {
        Self {
            pairs,
            has_one: true,
        }
    }

    pub(crate) fn node_count(&self) -> usize {
        self.pairs.len() + usize::from(self.has_one)
    }

    pub(crate) fn pairs(&self) -> &[[usize; 2]] {
        &self.pairs
    }

    pub(crate) fn pair_capacity(&self) -> usize {
        self.pairs.capacity()
    }

    pub(crate) fn node(&self, index: usize) -> Option<Node> {
        if index == 0 {
            return self.has_one.then_some(Node::One);
        }
        let &[left, right] = self.pairs.get(index - 1)?;
        Some(Node::Magma(left, right))
    }

    pub(crate) fn nodes(&self) -> impl ExactSizeIterator<Item = Node> + '_ {
        (0..self.node_count()).map(|index| self.node(index).expect("bounded logical index"))
    }

    pub(crate) fn initialize_one(&mut self) {
        self.has_one = true;
    }

    pub(crate) fn push(&mut self, pair: [usize; 2]) -> usize {
        let index = self.node_count();
        debug_assert!(self.has_one && pair.iter().all(|&child| child < index));
        self.pairs.push(pair);
        index
    }

    pub(crate) fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        self.pairs.try_reserve(additional)
    }
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
    table: PairTable,
    interned: HashMap<[usize; 2], usize>,
    max_nodes: usize,
}

impl Arena {
    /// El presupuesto cuenta nodos únicos de toda la arena, no sus usos.
    pub fn new(max_nodes: usize) -> Self {
        Self {
            identity: Arc::new(()),
            table: PairTable::default(),
            interned: HashMap::new(),
            max_nodes,
        }
    }

    /// Nodos lógicos únicos, incluidos One y los Magma construidos.
    pub fn node_count(&self) -> usize {
        self.table.node_count()
    }

    /// Inspección por copia en orden de construcción, sin asignar una colección.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Node> + '_ {
        self.table.nodes()
    }

    /// Copia de un nodo local; modificarla no altera la arena.
    /// Devuelve `None` para cualquier índice fuera de rango.
    pub fn node(&self, index: usize) -> Option<Node> {
        self.table.node(index)
    }

    /// Filas residentes de solo lectura: la fila k contiene los operandos del
    /// nodo lógico k+1. One ocupa el índice 0 sin fila cuando se ha construido;
    /// una arena nueva todavía no contiene One. Consultar [`Self::node_count`]
    /// para distinguir ambos casos y contar el presupuesto lógico.
    ///
    /// ```
    /// use emllm::expr::Arena;
    /// let mut arena = Arena::new(2);
    /// let one = arena.one().unwrap();
    /// arena.magma(&one, &one).unwrap();
    /// assert_eq!(arena.pairs(), &[[0, 0]]);
    /// assert_eq!(arena.node_count(), 2);
    /// ```
    ///
    /// La vista prestada no permite modificar operandos:
    ///
    /// ```compile_fail
    /// use emllm::expr::Arena;
    /// let mut arena = Arena::new(2);
    /// let one = arena.one().unwrap();
    /// arena.magma(&one, &one).unwrap();
    /// arena.pairs()[0] = [1, 1];
    /// ```
    pub fn pairs(&self) -> &[[usize; 2]] {
        self.table.pairs()
    }

    pub fn node_limit(&self) -> usize {
        self.max_nodes
    }

    pub fn one(&mut self) -> Result<NodeId, BuildError> {
        if self.node_count() == 0 {
            self.check_node_budget()?;
            self.table.initialize_one();
        }
        Ok(self.handle(0))
    }

    pub fn magma(&mut self, left: &NodeId, right: &NodeId) -> Result<NodeId, BuildError> {
        let left = self.validate_node(left)?;
        let right = self.validate_node(right)?;
        let pair = [left, right];
        if let Some(&index) = self.interned.get(&pair) {
            return Ok(self.handle(index));
        }
        self.check_node_budget()?;
        let index = self.table.push(pair);
        self.interned.insert(pair, index);
        Ok(self.handle(index))
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
        if id.index >= self.node_count() {
            return Err(BuildError::InvalidReference { index: id.index });
        }
        Ok(id.index)
    }

    fn check_node_budget(&self) -> Result<(), BuildError> {
        if self.node_count() >= self.max_nodes {
            return Err(BuildError::NodeLimit {
                limit: self.max_nodes,
            });
        }
        Ok(())
    }

    fn handle(&self, index: usize) -> NodeId {
        NodeId {
            arena: self.identity.clone(),
            index,
        }
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
        assert_eq!(arena.node_count(), 4);
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
        assert_eq!(first.node_count(), 1);
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
        assert_eq!(arena.node_count(), 1);
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
