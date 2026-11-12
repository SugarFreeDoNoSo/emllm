use emllm::expr::{Arena, Node};
use emllm::program::compile;

fn assert_exact_nodes(mut nodes: impl ExactSizeIterator<Item = Node>, expected: &[Node]) {
    for (index, &node) in expected.iter().enumerate() {
        let remaining = expected.len() - index;
        assert_eq!(nodes.len(), remaining);
        assert_eq!(nodes.size_hint(), (remaining, Some(remaining)));
        assert_eq!(nodes.next(), Some(node));
    }
    assert_eq!(nodes.len(), 0);
    assert_eq!(nodes.size_hint(), (0, Some(0)));
    assert_eq!(nodes.next(), None);
}

#[test]
fn empty_arena_has_no_logical_nodes() {
    let arena = Arena::new(3);
    assert_eq!(arena.node_count(), 0);
    assert_exact_nodes(arena.nodes(), &[]);
    for index in [0, 1, usize::MAX] {
        assert_eq!(arena.node(index), None);
    }
}

#[test]
fn logical_inspection_returns_exact_iterators_and_independent_copies() {
    let mut arena = Arena::new(3);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    let root = arena.magma(&t, &t).unwrap();
    let program = compile(&arena, &[root], 3).unwrap();
    let expected = [Node::One, Node::Magma(0, 0), Node::Magma(1, 1)];
    assert_eq!(arena.node_count(), expected.len());
    assert_eq!(program.node_count(), expected.len());
    assert_exact_nodes(arena.nodes(), &expected);
    assert_exact_nodes(program.nodes(), &expected);
    for (index, &node) in expected.iter().enumerate() {
        assert_eq!(arena.node(index), Some(node));
        assert_eq!(program.node(index), Some(node));
    }
    for index in [3, 4, usize::MAX] {
        assert_eq!(arena.node(index), None);
        assert_eq!(program.node(index), None);
    }

    let roots = program.roots().to_vec();
    let stats = *program.stats();
    for mut copy in [arena.node(2).unwrap(), program.node(2).unwrap()] {
        let Node::Magma(ref mut left, _) = copy else {
            panic!("expected a copied Magma");
        };
        *left = usize::MAX;
        assert_eq!(copy, Node::Magma(usize::MAX, 1));
        assert_eq!(arena.node(2), Some(expected[2]));
        assert_eq!(program.node(2), Some(expected[2]));
    }
    assert_eq!(program.roots(), roots);
    assert_eq!(program.stats(), &stats);
    assert_exact_nodes(arena.nodes(), &expected);
    assert_exact_nodes(program.nodes(), &expected);
}
