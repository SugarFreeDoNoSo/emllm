use emllm::{
    eval::{EvalError, EvalErrorKind, evaluate},
    execution::{PlanError, PlanLimits, plan, run},
    expr::Arena,
    program::compile,
};

const LIMITS: PlanLimits = PlanLimits {
    max_nodes: 100_000,
    max_slots: 100_000,
};

#[test]
fn one_is_reserved_and_limits_are_inclusive() {
    let mut arena = Arena::new(1);
    let one = arena.one().unwrap();
    let program = compile(&arena, &[one.clone(), one], 1).unwrap();
    for limits in [
        PlanLimits {
            max_nodes: 0,
            max_slots: 1,
        },
        PlanLimits {
            max_nodes: 1,
            max_slots: 0,
        },
    ] {
        let expected = if limits.max_nodes == 0 {
            PlanError::NodeLimit { limit: 0 }
        } else {
            PlanError::SlotLimit { limit: 0 }
        };
        assert_eq!(plan(&program, limits).unwrap_err(), expected);
    }
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: 1,
            max_slots: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(p.program(), &program));
    assert_eq!(p.stats().slots, 1);
    assert_eq!(p.stats().instructions, 0);
    assert_eq!(p.stats().instruction_payload_bytes, 0);
    assert_eq!(p.root_slots(), &[0, 0]);
}

#[test]
fn roots_stay_live_and_plan_is_deterministic() {
    let mut arena = Arena::new(3);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let both = arena.magma(&e, &e).unwrap();
    let program = compile(&arena, &[e.clone(), one, both, e], 3).unwrap();
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: 3,
            max_slots: 3,
        },
    )
    .unwrap();
    assert_eq!(p.root_slots(), &[1, 0, 2, 1]);
    assert_eq!(p.instructions().collect::<Vec<_>>(), [[0, 0, 1], [1, 1, 2]]);
    assert_eq!(p.stats(), plan(&program, LIMITS).unwrap().stats());
    assert!(
        p.instructions()
            .eq(plan(&program, LIMITS).unwrap().instructions())
    );
    assert_eq!(
        plan(
            &program,
            PlanLimits {
                max_slots: 2,
                ..LIMITS
            }
        )
        .unwrap_err(),
        PlanError::SlotLimit { limit: 2 }
    );
    assert_eq!(
        plan(
            &program,
            PlanLimits {
                max_nodes: 2,
                ..LIMITS
            }
        )
        .unwrap_err(),
        PlanError::NodeLimit { limit: 2 }
    );
}

#[test]
fn planning_a_numerically_invalid_program_does_not_evaluate_it() {
    let mut arena = Arena::new(5);
    let one = arena.one().unwrap();
    let mut root = one.clone();
    for _ in 0..4 {
        root = arena.magma(&root, &one).unwrap();
    }
    let program = compile(&arena, &[root], 5).unwrap();
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: 5,
            max_slots: 2,
        },
    )
    .unwrap();
    assert_eq!(p.stats().slots, 2);
    assert_eq!(
        p.instructions().collect::<Vec<_>>(),
        [[0, 0, 1], [1, 0, 1], [1, 0, 1], [1, 0, 1]]
    );
    assert_eq!(
        evaluate(&program, 5).unwrap_err().kind,
        EvalErrorKind::ExponentialOverflow
    );
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (&a, &b) in actual.iter().zip(expected) {
        assert!(a.is_finite() && b.is_finite());
        assert!((a - b).abs() <= 1e-12 + 1e-12 * b.abs(), "{a} != {b}");
    }
}

#[test]
fn execution_reads_both_operands_before_reuse_and_preserves_root_order() {
    let mut arena = Arena::new(3);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let both = arena.magma(&e, &e).unwrap();
    let program = compile(&arena, &[both.clone(), one.clone(), both], 3).unwrap();
    let p = plan(&program, LIMITS).unwrap();
    assert_eq!(p.instructions().nth(1), Some([1, 1, 1]));
    assert_eq!(p.root_slots(), &[1, 0, 1]);
    assert_close(&run(&p, 3).unwrap(), &evaluate(&program, 3).unwrap());
    let program = compile(&arena, &[e.clone(), one, e], 3).unwrap();
    let p = plan(&program, LIMITS).unwrap();
    let result = run(&p, 2).unwrap();
    assert_close(&result, &evaluate(&program, 2).unwrap());
    assert!((result[0] - std::f64::consts::E).abs() <= 1e-12);
    assert_eq!(result[0], result[2]);
}

#[test]
fn execution_preserves_domain_overflow_and_logical_budget_errors() {
    let mut arena = Arena::new(10);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let ee = arena.magma(&e, &one).unwrap();
    let large = arena.magma(&ee, &one).unwrap();
    let negative = arena.magma(&one, &large).unwrap();
    let domain = arena.magma(&one, &negative).unwrap();
    let overflow = arena.magma(&large, &one).unwrap();
    for (root, kind) in [
        (domain, EvalErrorKind::LogDomain),
        (overflow, EvalErrorKind::ExponentialOverflow),
    ] {
        let program = compile(&arena, &[one.clone(), root], 10).unwrap();
        let p = plan(
            &program,
            PlanLimits {
                max_slots: 2,
                ..LIMITS
            },
        )
        .unwrap();
        let error = run(&p, 10).unwrap_err();
        assert_eq!(error, evaluate(&program, 10).unwrap_err());
        assert_eq!(error.kind, kind);
        assert_eq!(error.node, Some(program.roots()[1]));
        assert!(program.node(error.node.unwrap()).is_some());
        for limit in [0, 2, program.node_count() - 1] {
            assert_eq!(
                run(&p, limit).unwrap_err(),
                EvalError {
                    kind: EvalErrorKind::NodeLimit { limit },
                    node: None
                }
            );
        }
    }
}
