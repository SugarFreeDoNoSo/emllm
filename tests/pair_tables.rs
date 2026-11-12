use emllm::eval::{EvalError, EvalErrorKind, evaluate};
use emllm::expr::{Arena, BuildError, Node};
use emllm::program::{CompileError, Program, compile};

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

#[test]
fn one_program_requires_a_logical_node_but_no_pair_row() {
    let mut arena = Arena::new(1);
    let one = arena.one().unwrap();
    assert!(matches!(
        compile(&arena, std::slice::from_ref(&one), 0),
        Err(CompileError::NodeLimit { limit: 0 })
    ));
    let program = compile(&arena, &[one], 1).unwrap();
    assert!(program.pairs().is_empty());
    assert_eq!(program.node_count(), 1);
    assert_eq!(program.roots(), &[0]);
    assert_eq!(program.node(0), Some(Node::One));
    assert!(program.nodes().eq([Node::One]));
    assert_eq!(program.node(1), None);
    assert_eq!(
        evaluate(&program, 0),
        Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: 0 },
            node: None,
        })
    );
    assert_eq!(evaluate(&program, 1).unwrap(), vec![1.0]);
}

#[test]
fn compilation_remaps_repeated_roots_and_excludes_unreachable_rows() {
    let mut arena = Arena::new(4);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    arena.magma(&one, &t).unwrap(); // Fila que debe excluirse.
    let v = arena.magma(&t, &t).unwrap();
    assert_eq!(arena.pairs(), &[[0, 0], [0, 1], [1, 1]]);
    assert_eq!(v.index(), 3);
    let roots = [v.clone(), one.clone(), v];
    let program = compile(&arena, &roots, 3).unwrap();
    assert_eq!(program.pairs(), &[[0, 0], [1, 1]]);
    assert_eq!(program.roots(), &[2, 0, 2]);
    assert_eq!(program.node_count(), 3);
    assert_eq!(program.node(2), Some(Node::Magma(1, 1)));
    assert_eq!(program.stats().roots, 3);

    assert!(matches!(
        compile(&arena, &roots, 2),
        Err(CompileError::NodeLimit { limit: 2 })
    ));
    assert!(matches!(
        compile(&arena, &[], 4),
        Err(CompileError::Roots(BuildError::EmptyRoots))
    ));
    let mut other = Arena::new(1);
    let foreign = other.one().unwrap();
    assert_eq!(foreign.index(), one.index());
    assert!(matches!(
        compile(&arena, &[foreign], 4),
        Err(CompileError::Roots(BuildError::ForeignReference))
    ));
    assert_eq!(program.pairs(), &[[0, 0], [1, 1]]);
    assert_eq!(program.roots(), &[2, 0, 2]);
}

#[test]
fn snapshots_own_their_pairs_after_arena_growth_and_destruction() {
    let mut arena = Arena::new(5);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    let parent = arena.magma(&t, &t).unwrap();
    let program = compile(&arena, &[parent.clone(), one.clone(), parent.clone()], 3).unwrap();
    let cloned = program.clone();
    let nodes = program.nodes().collect::<Vec<_>>();
    let stats = *program.stats();
    let inspect = |snapshot: &Program| {
        assert_eq!(snapshot.pairs(), &[[0, 0], [1, 1]]);
        assert_eq!(snapshot.roots(), &[2, 0, 2]);
        assert_eq!(snapshot.nodes().collect::<Vec<_>>(), nodes);
        assert_eq!(snapshot.stats(), &stats);
    };
    let more = arena.magma(&parent, &one).unwrap();
    arena.magma(&more, &parent).unwrap();
    assert_eq!(arena.node_count(), 5);
    inspect(&program);
    inspect(&cloned);
    drop(arena);
    inspect(&program);
    inspect(&cloned);
}
