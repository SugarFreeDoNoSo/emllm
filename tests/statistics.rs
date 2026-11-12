use emllm::expr::Arena;
use emllm::program::{ProgramStats, compile};

#[test]
fn counts_shared_operands_and_repeated_roots_by_position() {
    let mut arena = Arena::new(3);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    let parent = arena.magma(&t, &t).unwrap();
    for (roots, (nodes, edges, root_count, depth)) in [
        (vec![one], (1, 0, 1, 0)),
        (vec![t.clone()], (2, 2, 1, 1)),
        (vec![parent], (3, 4, 1, 2)),
        (vec![t.clone(), t], (2, 2, 2, 1)),
    ] {
        let program = compile(&arena, &roots, 3).unwrap();
        assert_eq!(program.node_count(), nodes);
        assert_eq!(program.pairs().len(), nodes - 1);
        assert_eq!(edges, 2 * program.pairs().len());
        assert_eq!(program.pairs(), &[[0, 0], [1, 1]][..nodes - 1]);
        assert_eq!(
            program.stats(),
            &ProgramStats {
                nodes,
                edges,
                roots: root_count,
                depth,
            }
        );
    }
}

#[test]
fn statistics_exclude_arena_history_and_survive_its_destruction() {
    let mut arena = Arena::new(5);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    let unused = arena.magma(&t, &one).unwrap();
    let program = compile(&arena, &[t], 2).unwrap();
    let expected = ProgramStats {
        nodes: 2,
        edges: 2,
        roots: 1,
        depth: 1,
    };
    assert_eq!(program.stats(), &expected);
    let more = arena.magma(&unused, &one).unwrap();
    arena.magma(&more, &one).unwrap();
    assert_eq!(arena.node_count(), 5);
    assert_eq!(program.stats(), &expected);
    drop(arena);
    assert_eq!(program.stats(), &expected);
}
