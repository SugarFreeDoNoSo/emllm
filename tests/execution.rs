use emllm::{
    eval::{EvalError, EvalErrorKind, evaluate},
    execution::{ExecutionPlan, PlanError, PlanLimits, plan, run},
    expr::{Arena, Node},
    program::{Program, ProgramStats, compile},
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

fn snapshot(program: &Program) -> (Vec<Node>, Vec<[usize; 2]>, Vec<usize>, ProgramStats) {
    (
        program.nodes().collect(),
        program.pairs().to_vec(),
        program.roots().to_vec(),
        *program.stats(),
    )
}

// Oráculo estructural independiente: cada posición guarda la identidad lógica,
// no un valor. Detecta liberaciones prematuras aun cuando el valor coincida.
fn check_live_identities(p: &ExecutionPlan<'_>) {
    let mut identities = vec![None; p.stats().slots];
    identities[0] = Some(0);
    for (row, ([left, right, destination], &[a, b])) in
        p.instructions().zip(p.program().pairs()).enumerate()
    {
        assert_eq!(identities[left], Some(a));
        assert_eq!(identities[right], Some(b));
        assert_ne!(destination, 0);
        identities[destination] = Some(row + 1);
    }
    for (&slot, &root) in p.root_slots().iter().zip(p.program().roots()) {
        assert_eq!(identities[slot], Some(root));
    }
}

#[test]
fn diamond_and_simultaneous_last_uses_preserve_intermediate_roots() {
    let mut arena = Arena::new(5);
    let one = arena.one().unwrap();
    let shared = arena.magma(&one, &one).unwrap();
    let left = arena.magma(&one, &shared).unwrap();
    let right = arena.magma(&shared, &shared).unwrap();
    let root = arena.magma(&left, &right).unwrap();
    for roots in [vec![root.clone()], vec![shared.clone(), root, shared]] {
        let program = compile(&arena, &roots, 5).unwrap();
        let before = snapshot(&program);
        let p = plan(&program, LIMITS).unwrap();
        check_live_identities(&p);
        let [l, r, d] = p.instructions().last().unwrap();
        assert_ne!(l, r);
        assert!(d == l || d == r);
        assert_close(&run(&p, 5).unwrap(), &evaluate(&program, 5).unwrap());
        assert_eq!(snapshot(&program), before);
    }
}

#[test]
fn stable_ten_thousand_step_chain_uses_two_slots() {
    let mut arena = Arena::new(10_001);
    let one = arena.one().unwrap();
    let mut root = one.clone();
    for _ in 0..10_000 {
        root = arena.magma(&one, &root).unwrap();
    }
    let program = compile(&arena, &[root], 10_001).unwrap();
    drop(arena);
    let before = snapshot(&program);
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: 10_001,
            max_slots: 2,
        },
    )
    .unwrap();
    assert_eq!(program.node_count(), 10_001);
    assert_eq!(p.stats().slots, 2);
    assert_eq!(p.stats().value_bytes, 16);
    check_live_identities(&p);
    assert_close(
        &run(&p, 10_001).unwrap(),
        &evaluate(&program, 10_001).unwrap(),
    );
    assert_eq!(
        plan(
            &program,
            PlanLimits {
                max_slots: 1,
                ..LIMITS
            }
        )
        .unwrap_err(),
        PlanError::SlotLimit { limit: 1 }
    );
    assert_eq!(run(&p, 10_000).unwrap_err().node, None);
    assert_eq!(snapshot(&program), before);
}

#[test]
fn failures_retries_and_interleaved_runs_preserve_every_snapshot() {
    let mut arena = Arena::new(12);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let stable = arena.magma(&one, &e).unwrap();
    let ee = arena.magma(&e, &one).unwrap();
    let large = arena.magma(&ee, &one).unwrap();
    let negative = arena.magma(&one, &large).unwrap();
    let domain = arena.magma(&one, &negative).unwrap();
    let overflow = arena.magma(&large, &one).unwrap();
    let good = compile(&arena, &[stable.clone(), one.clone(), stable], 12).unwrap();
    let bad = [
        compile(&arena, &[domain.clone(), overflow.clone()], 12).unwrap(),
        compile(&arena, &[overflow, domain], 12).unwrap(),
    ];
    drop(arena);
    let before = snapshot(&good);
    let p = plan(&good, LIMITS).unwrap();
    let stats = p.stats();
    let instructions = p.instructions().collect::<Vec<_>>();
    let expected = evaluate(&good, 12).unwrap();
    for invalid in bad {
        let before_bad = snapshot(&invalid);
        let failing = plan(&invalid, LIMITS).unwrap();
        check_live_identities(&failing);
        assert!(failing.stats().slots < invalid.node_count());
        for _ in 0..3 {
            assert_close(&run(&p, 12).unwrap(), &expected);
            assert_eq!(run(&failing, 12), evaluate(&invalid, 12));
            assert_eq!(run(&p, 0).unwrap_err().node, None);
            assert_close(&run(&p, 12).unwrap(), &expected);
        }
        assert_eq!(snapshot(&invalid), before_bad);
    }
    assert_eq!(snapshot(&good), before);
    assert_eq!(p.stats(), stats);
    assert!(p.instructions().eq(instructions));
}

#[test]
fn deterministic_shared_dags_preserve_logical_identities_and_first_errors() {
    for seed in 0..32 {
        let mut arena = Arena::new(201);
        let mut nodes = vec![arena.one().unwrap()];
        for i in 0..200 {
            let left = (i * 17 + seed) % nodes.len();
            let right = (i * 7 + seed * 13) % nodes.len();
            nodes.push(arena.magma(&nodes[left], &nodes[right]).unwrap());
        }
        let roots = (0..nodes.len())
            .step_by(13)
            .map(|i| nodes[i].clone())
            .collect::<Vec<_>>();
        let program = compile(&arena, &roots, 201).unwrap();
        let before = snapshot(&program);
        let p = plan(&program, LIMITS).unwrap();
        check_live_identities(&p);
        match (run(&p, 201), evaluate(&program, 201)) {
            (Ok(actual), Ok(expected)) => assert_close(&actual, &expected),
            (Err(actual), Err(expected)) => assert_eq!(actual, expected),
            other => panic!("differential mismatch: {other:?}"),
        }
        assert_eq!(snapshot(&program), before);
    }
}
