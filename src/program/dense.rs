//! Compilación de vistas densas internas; no abre un constructor público de pares.
use super::{BuildError, CompileError, Node, PairTable, Program, ProgramStats};

#[derive(Debug)]
pub(crate) enum DenseCompileError {
    Graph(CompileError),
    Allocation { storage: &'static str },
    SizeOverflow { storage: &'static str },
}
impl From<CompileError> for DenseCompileError {
    fn from(error: CompileError) -> Self {
        Self::Graph(error)
    }
}

/// Temporales observados al compilar; se liberan antes de devolver el programa.
/// Estados/remapeo cubren toda la fuente, profundidad solo la clausura alcanzada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DenseCompileStorage {
    pub source_nodes: usize,
    pub reachable_nodes: usize,
    pub state_capacity_bytes: usize,
    pub remap_capacity_bytes: usize,
    pub depth_capacity_bytes: usize,
    pub stack_capacity_bytes: usize,
}

fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    storage: &'static str,
) -> Result<(), DenseCompileError> {
    values
        .len()
        .checked_add(additional)
        .and_then(|n| n.checked_mul(size_of::<T>()))
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or(DenseCompileError::SizeOverflow { storage })?;
    values
        .try_reserve(additional)
        .map_err(|_| DenseCompileError::Allocation { storage })
}

pub(crate) fn compile_dense(
    source: impl Fn(usize) -> Option<Node>,
    source_nodes: usize,
    roots: &[usize],
    max_nodes: usize,
) -> Result<(Program, DenseCompileStorage), DenseCompileError> {
    if roots.is_empty() {
        return Err(CompileError::Roots(BuildError::EmptyRoots).into());
    }
    if max_nodes == 0 {
        return Err(CompileError::NodeLimit { limit: 0 }.into());
    }
    let mut states: Vec<u8> = Vec::new();
    reserve(&mut states, source_nodes, "dense states")?;
    states.resize(source_nodes, 0);
    let mut remap = Vec::new();
    reserve(&mut remap, source_nodes, "dense remap")?;
    remap.resize(source_nodes, usize::MAX);
    let mut depths: Vec<usize> = Vec::new();
    let mut stack = Vec::new();
    let mut table = PairTable::default();
    let mut visited = 0;
    for &root in roots {
        reserve(&mut stack, 1, "dense stack")?;
        stack.push((root, false));
        while let Some((index, exiting)) = stack.pop() {
            if index >= source_nodes {
                return Err(CompileError::InvalidReference { index }.into());
            }
            let node = source(index).ok_or(CompileError::InvalidReference { index })?;
            if exiting {
                reserve(&mut depths, 1, "dense depth")?;
                let (compiled, depth) = match node {
                    Node::One => {
                        if index != 0 {
                            return Err(CompileError::InvalidReference { index }.into());
                        }
                        table.initialize_one();
                        (0, 0)
                    }
                    Node::Magma(left, right) => {
                        let (left, right) = (remap[left], remap[right]);
                        table
                            .try_reserve(1)
                            .map_err(|_| DenseCompileError::Allocation {
                                storage: "compiled pairs",
                            })?;
                        let depth = 1usize
                            .checked_add(depths[left].max(depths[right]))
                            .ok_or(DenseCompileError::SizeOverflow { storage: "depth" })?;
                        (table.push([left, right]), depth)
                    }
                };
                remap[index] = compiled;
                depths.push(depth);
                states[index] = 2;
                continue;
            }
            match states[index] {
                2 => continue,
                1 => return Err(CompileError::Cycle { index }.into()),
                _ => {}
            }
            if visited >= max_nodes {
                return Err(CompileError::NodeLimit { limit: max_nodes }.into());
            }
            visited += 1;
            states[index] = 1;
            reserve(&mut stack, 3, "dense stack")?;
            stack.push((index, true));
            if let Node::Magma(left, right) = node {
                stack.push((right, false));
                stack.push((left, false));
            }
        }
    }
    let mut compiled_roots = Vec::new();
    reserve(&mut compiled_roots, roots.len(), "compiled roots")?;
    compiled_roots.extend(roots.iter().map(|&root| remap[root]));
    let storage = DenseCompileStorage {
        source_nodes,
        reachable_nodes: table.node_count(),
        state_capacity_bytes: states.capacity(),
        remap_capacity_bytes: remap.capacity() * size_of::<usize>(),
        depth_capacity_bytes: depths.capacity() * size_of::<usize>(),
        stack_capacity_bytes: stack.capacity() * size_of::<(usize, bool)>(),
    };
    let edges = table
        .pairs()
        .len()
        .checked_mul(2)
        .ok_or(DenseCompileError::SizeOverflow {
            storage: "edge count",
        })?;
    let stats = ProgramStats {
        nodes: table.node_count(),
        edges,
        roots: roots.len(),
        depth: depths.into_iter().max().unwrap_or(0),
    };
    Ok((
        Program {
            table,
            roots: compiled_roots,
            stats,
        },
        storage,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expr::Arena,
        program::{compile, compile_nodes},
    };

    #[test]
    fn dense_matches_existing_order_sharing_repeated_roots_and_unreachable_nodes() {
        let mut arena = Arena::new(20_010);
        let one = arena.one().unwrap();
        let mut root = one.clone();
        for _ in 0..20_000 {
            root = arena.magma(&root, &root).unwrap();
        }
        arena.magma(&one, &root).unwrap();
        let roots = [root.clone(), one, root];
        let indices = roots.iter().map(|r| r.index()).collect::<Vec<_>>();
        let expected = compile(&arena, &roots, 20_001).unwrap();
        let (actual, storage) =
            compile_dense(|i| arena.node(i), arena.node_count(), &indices, 20_001).unwrap();
        assert_eq!(actual.pairs(), expected.pairs());
        assert_eq!(actual.roots(), expected.roots());
        assert_eq!(actual.stats(), expected.stats());
        assert_eq!(storage.source_nodes, 20_002);
        assert_eq!(storage.reachable_nodes, 20_001);
        let (one, _) = compile_dense(|i| arena.node(i), arena.node_count(), &[0, 0], 1).unwrap();
        assert_eq!(one.node_count(), 1);

        // El recorrido depende de las raíces y del orden izquierdo/derecho,
        // no del orden de inserción original de ramas independientes.
        let graph = PairTable::from_test_pairs(vec![[0, 0], [1, 0], [0, 1], [3, 2]]);
        let expected = compile_nodes(|i| graph.node(i), &[4, 3, 4], 5).unwrap();
        let (actual, _) = compile_dense(|i| graph.node(i), 5, &[4, 3, 4], 5).unwrap();
        assert_eq!(actual.pairs(), expected.pairs());
        assert_eq!(actual.roots(), expected.roots());
        assert_eq!(actual.pairs(), &[[0, 0], [0, 1], [1, 0], [2, 3]]);
    }

    #[test]
    fn dense_reports_the_same_graph_errors_and_inclusive_budget() {
        for pairs in [vec![[2, 0]], vec![[1, 0]], vec![[2, 0], [1, 0]]] {
            let graph = PairTable::from_test_pairs(pairs);
            let expected = compile_nodes(|i| graph.node(i), &[1], 4).unwrap_err();
            let error = compile_dense(|i| graph.node(i), graph.node_count(), &[1], 4).unwrap_err();
            assert!(matches!(error, DenseCompileError::Graph(actual) if actual == expected));
        }
        let graph = PairTable::from_test_pairs(vec![[0, 0]]);
        assert!(compile_dense(|i| graph.node(i), 2, &[1], 2).is_ok());
        assert!(matches!(
            compile_dense(|i| graph.node(i), 2, &[1], 0).unwrap_err(),
            DenseCompileError::Graph(CompileError::NodeLimit { limit: 0 })
        ));
        assert!(matches!(
            compile_dense(|i| graph.node(i), 2, &[1], 1).unwrap_err(),
            DenseCompileError::Graph(CompileError::NodeLimit { limit: 1 })
        ));
        assert!(matches!(
            compile_dense(|i| graph.node(i), 2, &[], 2).unwrap_err(),
            DenseCompileError::Graph(CompileError::Roots(BuildError::EmptyRoots))
        ));
        let mut vector = Vec::<usize>::new();
        assert!(matches!(
            reserve(&mut vector, usize::MAX, "fixture"),
            Err(DenseCompileError::SizeOverflow { .. })
        ));
    }
}
