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
