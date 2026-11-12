use emllm::expr::{Arena, BuildError, Node};

#[test]
fn arena_stores_magma_pairs_with_lazy_implicit_one() {
    let mut arena = Arena::new(3);
    assert!(arena.pairs().is_empty());
    assert_eq!(arena.node(0), None);
    assert_eq!(arena.node_count(), 0);

    let one = arena.one().unwrap();
    assert_eq!(one.index(), 0);
    assert!(arena.pairs().is_empty());
    assert_eq!(arena.node(0), Some(Node::One));
    assert_eq!(arena.node_count(), 1);
    assert_eq!(arena.one().unwrap(), one);

    let t = arena.magma(&one, &one).unwrap();
    let parent = arena.magma(&t, &t).unwrap();
    assert_eq!(t.index(), 1);
    assert_eq!(parent.index(), 2);
    assert_eq!(arena.pairs(), &[[0, 0], [1, 1]]);
    assert_eq!(arena.node_count(), 3);
    assert!(
        arena
            .nodes()
            .eq([Node::One, Node::Magma(0, 0), Node::Magma(1, 1)])
    );
}

#[test]
fn interned_pairs_reuse_rows_and_preserve_operand_order() {
    let mut arena = Arena::new(4);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    let left = arena.magma(&one, &t).unwrap();
    let right = arena.magma(&t, &one).unwrap();
    assert_ne!(left, right);
    assert_eq!(arena.magma(&one, &t).unwrap(), left);
    assert_eq!(arena.magma(&t, &one).unwrap(), right);
    assert_eq!(arena.pairs(), &[[0, 0], [0, 1], [1, 0]]);
    assert_eq!(arena.node_count(), 4);
}

#[test]
fn construction_budgets_count_one_even_without_a_row() {
    let mut empty = Arena::new(0);
    assert_eq!(empty.one(), Err(BuildError::NodeLimit { limit: 0 }));
    assert!(empty.pairs().is_empty());
    assert_eq!(empty.node_count(), 0);
    assert_eq!(empty.node(0), None);

    let mut arena = Arena::new(1);
    let one = arena.one().unwrap();
    assert_eq!(arena.one().unwrap(), one);
    assert_eq!(arena.node_count(), 1);
    assert_eq!(
        arena.magma(&one, &one),
        Err(BuildError::NodeLimit { limit: 1 })
    );
    assert!(arena.pairs().is_empty());
    assert_eq!(arena.node(0), Some(Node::One));
}
