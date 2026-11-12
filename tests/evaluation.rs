use emllm::eval::{EvalError, EvalErrorKind, evaluate};
use emllm::expr::Arena;
use emllm::program::compile;

#[test]
fn evaluates_elementary_and_shared_expressions() {
    let mut arena = Arena::new(10);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let combined = arena.magma(&e, &e).unwrap();
    let program = compile(&arena, &[one, e.clone(), combined, e], 10).unwrap();
    assert_eq!(program.node_count(), 3);
    let values = evaluate(&program, 10).unwrap();
    assert_eq!(values[0], 1.0);
    assert!((values[1] - std::f64::consts::E).abs() <= 1e-12);
    assert!((values[2] - (std::f64::consts::E.exp() - 1.0)).abs() <= 1e-12);
    assert_eq!(values[1], values[3]);
}

#[test]
fn numerical_failures_report_the_compiled_node() {
    let mut arena = Arena::new(10);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let ee = arena.magma(&e, &one).unwrap();
    let large = arena.magma(&ee, &one).unwrap();
    let negative = arena.magma(&one, &large).unwrap();
    let invalid = arena.magma(&one, &negative).unwrap();
    let overflow = arena.magma(&large, &one).unwrap();

    for (root, kind) in [
        (invalid, EvalErrorKind::LogDomain),
        (overflow, EvalErrorKind::ExponentialOverflow),
    ] {
        let program = compile(&arena, &[one.clone(), root], 10).unwrap();
        let nodes = program.nodes().collect::<Vec<_>>();
        let roots = program.roots().to_vec();
        let stats = *program.stats();
        assert_eq!(
            evaluate(&program, 10),
            Err(EvalError {
                kind,
                node: Some(program.roots()[1]),
            })
        );
        assert_eq!(program.nodes().collect::<Vec<_>>(), nodes);
        assert_eq!(program.roots(), roots);
        assert_eq!(program.stats(), &stats);
        assert_eq!(program.node(roots[1]), Some(nodes[roots[1]]));
    }
}

#[test]
fn rejects_insufficient_evaluation_budget_before_executing_nodes() {
    let mut arena = Arena::new(2);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let program = compile(&arena, &[e], 2).unwrap();
    assert_eq!(
        evaluate(&program, 1),
        Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: 1 },
            node: None,
        })
    );
    assert!((evaluate(&program, 2).unwrap()[0] - std::f64::consts::E).abs() <= 1e-12);
}
