//! Validación estructural y compilación iterativa de las raíces seleccionadas.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, NodeId, PairTable};

mod dense;
pub use dense::DenseCompileStorage;
pub(crate) use dense::{DenseCompileError, compile_dense};

/// Forma cerrada de los nodos que expone la inspección del programa.
pub use crate::expr::Node;

/// Snapshot cerrado cuyos hijos preceden a sus padres en `nodes()`.
///
/// Posee su tabla de pares y raíces: ampliar o liberar la arena no lo cambia.
/// One está implícito en el índice 0; la fila k representa el nodo k+1.
/// La compilación validada lo produce desde una construcción. Clonarlo copia un
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
/// assert!(program.nodes().eq([Node::One, Node::Magma(0, 0)]));
/// assert_eq!(program.pairs(), &[[0, 0]]);
/// assert_eq!(program.roots(), &[1]);
/// assert!((evaluate(&program, 2).unwrap()[0] - std::f64::consts::E).abs() <= 1e-12);
/// ```
///
/// Los campos privados impiden fabricar otro programa saltándose la validación:
///
/// ```compile_fail
/// use emllm::{expr::Arena, program::{Program, compile}};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let program = compile(&arena, &[one], 1).unwrap();
/// let invalid = Program { roots: vec![99], ..program };
/// ```
///
/// Ni las filas ni las raíces ofrecen acceso mutable:
///
/// ```compile_fail
/// use emllm::{expr::Arena, program::compile};
/// let mut arena = Arena::new(2);
/// let one = arena.one().unwrap();
/// let root = arena.magma(&one, &one).unwrap();
/// let mut program = compile(&arena, &[root], 2).unwrap();
/// program.pairs()[0] = [1, 1];
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
    table: PairTable,
    roots: Vec<usize>,
    stats: ProgramStats,
}

/// Estadísticas de la clausura alcanzable, calculadas al compilar.
/// No cuentan nodos de la arena fuera del programa ni evalúan exp/ln.
/// La consulta permanece estable aunque se libere la arena o falle la evaluación.
/// Son conteos estructurales, no medidas de bytes, tiempo o complejidad del código.
///
/// ```
/// use emllm::{expr::Arena, program::{ProgramStats, compile}};
/// let mut arena = Arena::new(3);
/// let one = arena.one().unwrap();
/// let t = arena.magma(&one, &one).unwrap();
/// let root = arena.magma(&t, &t).unwrap();
/// let program = compile(&arena, &[root], 3).unwrap();
/// assert_eq!(program.stats(), &ProgramStats { nodes: 3, edges: 4, roots: 1, depth: 2 });
/// ```
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

/// Almacenamiento residente del snapshot. Capacidad incluye payload y espacio
/// sobrante; metadata incluye cabeceras y padding. No mide asignador ni RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramStorage {
    pub pair_payload_bytes: usize,
    pub pair_capacity_bytes: usize,
    pub root_payload_bytes: usize,
    pub root_capacity_bytes: usize,
    pub metadata_bytes: usize,
}

impl Program {
    /// Consulta O(1) de tamaños reales, sin modificar ni recorrer el snapshot.
    pub fn storage(&self) -> ProgramStorage {
        ProgramStorage {
            pair_payload_bytes: std::mem::size_of_val(self.pairs()),
            pair_capacity_bytes: self.table.pair_capacity() * std::mem::size_of::<[usize; 2]>(),
            root_payload_bytes: std::mem::size_of_val(self.roots()),
            root_capacity_bytes: self.roots.capacity() * std::mem::size_of::<usize>(),
            metadata_bytes: std::mem::size_of::<Self>(),
        }
    }

    /// Consulta constante de estadísticas, sin evaluación ni recorrido adicional.
    pub fn stats(&self) -> &ProgramStats {
        &self.stats
    }

    /// Número lógico de nodos del snapshot, incluido One.
    pub fn node_count(&self) -> usize {
        self.table.node_count()
    }

    /// Copias en orden de evaluación, sin asignar una colección de nodos.
    /// Sus posiciones y operandos son locales a este snapshot.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Node> + '_ {
        self.table.nodes()
    }

    /// Resuelve una posición de raíz, operando o diagnóstico en este programa.
    /// No acepta un handle de construcción [`NodeId`]; sus índices pueden haber
    /// sido remapeados. Un índice fuera del snapshot devuelve `None`.
    /// Modificar la copia devuelta no cambia el programa.
    pub fn node(&self, index: usize) -> Option<Node> {
        self.table.node(index)
    }

    /// Tabla residente de solo lectura, una fila por Magma único alcanzable.
    /// La fila k describe el nodo lógico k+1, con hijos anteriores a ese nodo.
    /// One siempre existe en el índice 0, incluso cuando no hay filas.
    ///
    /// ```
    /// use emllm::{expr::{Arena, Node}, program::compile};
    /// let mut arena = Arena::new(1);
    /// let one = arena.one().unwrap();
    /// let program = compile(&arena, &[one], 1).unwrap();
    /// drop(arena);
    /// assert!(program.pairs().is_empty());
    /// assert_eq!(program.node_count(), 1);
    /// assert_eq!(program.node(0), Some(Node::One));
    /// assert_eq!(program.roots(), &[0]);
    /// ```
    pub fn pairs(&self) -> &[[usize; 2]] {
        self.table.pairs()
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
    compile_nodes(|index| arena.node(index), &roots, max_nodes)
}

pub(crate) fn compile_nodes(
    source: impl Fn(usize) -> Option<Node>,
    roots: &[usize],
    max_nodes: usize,
) -> Result<Program, CompileError> {
    let mut states = HashMap::new();
    let mut remapped = HashMap::new();
    let mut table = PairTable::default();
    let mut depths: Vec<usize> = Vec::new();
    let mut stack = Vec::new();

    for &root in roots {
        stack.push((root, false));
        while let Some((index, exiting)) = stack.pop() {
            let node = source(index).ok_or(CompileError::InvalidReference { index })?;
            if exiting {
                let (compiled, depth) = match node {
                    Node::One => {
                        table.initialize_one();
                        (0, 0)
                    }
                    Node::Magma(left, right) => {
                        let left = remapped[&left];
                        let right = remapped[&right];
                        (
                            table.push([left, right]),
                            1 + depths[left].max(depths[right]),
                        )
                    }
                };
                remapped.insert(index, compiled);
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
            nodes: table.node_count(),
            edges: 2 * table.pairs().len(),
            roots: roots.len(),
            depth: depths.into_iter().max().unwrap_or(0),
        },
        table,
        roots: roots.iter().map(|root| remapped[root]).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_graphs() {
        for (pairs, expected) in [
            (vec![[2, 0]], CompileError::InvalidReference { index: 2 }),
            (vec![[1, 0]], CompileError::Cycle { index: 1 }),
            (vec![[2, 0], [1, 0]], CompileError::Cycle { index: 1 }),
        ] {
            let source = PairTable::from_test_pairs(pairs);
            let result = compile_nodes(|index| source.node(index), &[1], 3);
            assert_eq!(result.unwrap_err(), expected);
        }
    }

    #[test]
    fn compiles_deep_shared_graphs_without_expanding_the_tree() {
        let mut arena = Arena::new(20_001);
        let mut root = arena.one().unwrap();
        for _ in 0..20_000 {
            root = arena.magma(&root, &root).unwrap();
        }
        assert_eq!(arena.pairs().len(), 20_000);
        let program = compile(&arena, &[root], 20_001).unwrap();
        drop(arena);
        assert_eq!(program.pairs().len(), 20_000);
        assert_eq!(program.node_count(), 20_001);
        assert_eq!(program.nodes().len(), 20_001);
        assert_eq!(program.roots(), &[20_000]);
        assert_eq!(
            program.stats(),
            &ProgramStats {
                nodes: 20_001,
                edges: 40_000,
                roots: 1,
                depth: 20_000,
            }
        );
        for (index, node) in program.nodes().enumerate() {
            if let Node::Magma(left, right) = node {
                assert_eq!([left, right], [index - 1, index - 1]);
                assert_eq!(program.pairs()[index - 1], [left, right]);
            }
        }
    }

    #[test]
    fn compilation_budget_counts_reachable_nodes() {
        let mut arena = Arena::new(3);
        let one = arena.one().unwrap();
        let e = arena.magma(&one, &one).unwrap();
        arena.magma(&e, &one).unwrap();
        assert_eq!(compile(&arena, &[one], 1).unwrap().node_count(), 1);
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
