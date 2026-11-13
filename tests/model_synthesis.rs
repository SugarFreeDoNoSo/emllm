use emllm::{
    eval::evaluate,
    execution::{PlanLimits, plan, run},
    expr::{Arena, BuildError, Node},
    program::compile,
    synth::{
        Rational, RationalError,
        scalar::{F32Bits, decode, synthesize},
    },
};

#[test]
fn metadata_decodes_normals_subnormals_and_signed_zeros_exactly() {
    for bits in [
        0, 0x80000000, 1, 0x007fffff, 0x00800000, 0x3f000000, 0x3f800000, 0xbf400000, 0x7f7fffff,
    ] {
        let value = F32Bits::new(bits).unwrap();
        let decoded = decode(value);
        assert_eq!(decoded.negative, bits >> 31 != 0);
        assert!(decoded.mantissa == 0 || decoded.mantissa % 2 == 1);
        let magnitude = f64::from(decoded.mantissa) * 2.0_f64.powi(decoded.exponent);
        assert_eq!(
            if decoded.negative {
                -magnitude
            } else {
                magnitude
            },
            value.to_f64()
        );
    }
    for bits in [0x7f800000, 0xff800000, 0x7fc00001] {
        assert!(F32Bits::new(bits).is_none());
    }
    assert!(matches!(
        Rational::new(1, 3),
        Err(RationalError::OutsideDomain { .. })
    ));
}

#[test]
fn local_scalars_have_only_closed_nodes_and_match_both_evaluators() {
    for value in [
        0.0_f32,
        -0.0,
        0.5,
        -0.5,
        1.0,
        -1.0,
        1.5,
        1e-5,
        -1e-5,
        1e-20,
        7.0,
        -48.261856,
        2.781185e12,
    ] {
        let bits = F32Bits::new(value.to_bits()).unwrap();
        let mut arena = Arena::new(10_000);
        let root = synthesize(&mut arena, bits).unwrap();
        let program = compile(&arena, &[root], 10_000).unwrap();
        drop(arena);
        for (index, node) in program.nodes().enumerate() {
            match node {
                Node::One => assert_eq!(index, 0),
                Node::Magma(a, b) => assert!(a < index && b < index),
            }
        }
        let p = plan(
            &program,
            PlanLimits {
                max_nodes: 10_000,
                max_slots: 10_000,
            },
        )
        .unwrap();
        let expected = bits.to_f64();
        let original = evaluate(&program, 10_000).unwrap()[0];
        let planned = run(&p, 10_000).unwrap()[0];
        assert!(
            (original - expected).abs() <= 1e-9 + 1e-7 * expected.abs(),
            "bits={:08x}, actual={original}, expected={expected}",
            bits.bits()
        );
        assert!((planned - original).abs() <= 1e-12 + 1e-12 * original.abs());
    }
}

#[test]
fn construction_budgets_and_provenance_are_not_bypassed() {
    let value = F32Bits::new(0.125_f32.to_bits()).unwrap();
    let mut full = Arena::new(10_000);
    let root = synthesize(&mut full, value).unwrap();
    let exact = full.node_count();
    let mut limited = Arena::new(exact - 1);
    assert_eq!(
        synthesize(&mut limited, value),
        Err(BuildError::NodeLimit { limit: exact - 1 })
    );
    let one = limited.one().unwrap();
    assert_eq!(
        limited.magma(&one, &root),
        Err(BuildError::ForeignReference)
    );
    let mut enough = Arena::new(exact);
    synthesize(&mut enough, value).unwrap();
    assert_eq!(enough.node_count(), exact);
    assert_eq!(
        synthesize(&mut Arena::new(0), value),
        Err(BuildError::NodeLimit { limit: 0 })
    );
}

#[test]
fn candidate_reciprocal_root_and_silu_match_small_independent_cases() {
    use emllm::synth::model;
    for x in [0.5_f32, 1.0, 1.5] {
        let mut arena = Arena::new(10_000);
        let input = synthesize(&mut arena, F32Bits::new(x.to_bits()).unwrap()).unwrap();
        let reciprocal = model::reciprocal_positive(&mut arena, &input).unwrap();
        let inverse_root = model::inverse_sqrt(&mut arena, &input).unwrap();
        let silu = model::silu(&mut arena, &input).unwrap();
        let program = compile(&arena, &[reciprocal, inverse_root, silu], 10_000).unwrap();
        let p = plan(
            &program,
            PlanLimits {
                max_nodes: 10_000,
                max_slots: 10_000,
            },
        )
        .unwrap();
        let x = f64::from(x);
        let expected = [1.0 / x, 1.0 / x.sqrt(), x / (1.0 + (-x).exp())];
        let original = evaluate(&program, 10_000).unwrap_or_else(|error| {
            panic!("candidate x={x}, roots={:?}: {error}", program.roots())
        });
        for ((actual, optimized), reference) in original
            .into_iter()
            .zip(run(&p, 10_000).unwrap())
            .zip(expected)
        {
            assert!((actual - reference).abs() <= 1e-6 + 1e-5 * reference.abs());
            assert!((optimized - actual).abs() <= 1e-12 + 1e-12 * actual.abs());
        }
    }
}

#[test]
fn shifted_arithmetic_handles_signs_cancellation_and_historical_overflow() {
    use emllm::synth::model;
    let blocker = f32::from_bits(0x3f8a_fc5f);
    for (x, y) in [
        (blocker, blocker),
        (7.0, 0.0),
        (-32.0, 7.0),
        (7.0, -32.0),
        (32.0, 32.0),
        (-0.5, -0.5),
        (1e-5, -1e-5),
        (0.0, 0.0),
    ] {
        let mut arena = Arena::new(10_000);
        let a = synthesize(&mut arena, F32Bits::new(x.to_bits()).unwrap()).unwrap();
        let b = synthesize(&mut arena, F32Bits::new(y.to_bits()).unwrap()).unwrap();
        let roots = [
            model::add(&mut arena, &a, &b).unwrap(),
            model::sub(&mut arena, &a, &b).unwrap(),
            model::mul(&mut arena, &a, &b).unwrap(),
            model::neg(&mut arena, &a).unwrap(),
        ];
        let program = compile(&arena, &roots, 10_000).unwrap();
        drop(arena);
        let p = plan(
            &program,
            PlanLimits {
                max_nodes: 10_000,
                max_slots: 10_000,
            },
        )
        .unwrap();
        let (x, y) = (f64::from(x), f64::from(y));
        let expected = [x + y, x - y, x * y, -x];
        let original = evaluate(&program, 10_000).unwrap();
        let planned = run(&p, 10_000).unwrap();
        for ((actual, optimized), reference) in original.into_iter().zip(planned).zip(expected) {
            assert!(
                (actual - reference).abs() <= 1e-6 + 1e-5 * reference.abs(),
                "x={x}, y={y}, actual={actual}, expected={reference}"
            );
            assert!((optimized - actual).abs() <= 1e-12 + 1e-12 * actual.abs());
        }
    }
}

#[test]
fn candidate_operations_propagate_foreign_handles_budgets_and_domain_errors() {
    use emllm::{eval::EvalErrorKind, synth::model};
    let mut origin = Arena::new(10_000);
    let negative = synthesize(&mut origin, F32Bits::new((-1.0_f32).to_bits()).unwrap()).unwrap();
    let mut other = Arena::new(10_000);
    assert_eq!(
        model::reciprocal_positive(&mut other, &negative),
        Err(BuildError::ForeignReference)
    );
    let invalid = model::reciprocal_positive(&mut origin, &negative).unwrap();
    let program = compile(&origin, &[invalid], 10_000).unwrap();
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: 10_000,
            max_slots: 10_000,
        },
    )
    .unwrap();
    let error = evaluate(&program, 10_000).unwrap_err();
    assert_eq!(error.kind, EvalErrorKind::LogDomain);
    assert_eq!(run(&p, 10_000).unwrap_err(), error);
    let mut limited = Arena::new(1);
    let one = limited.one().unwrap();
    assert_eq!(
        model::silu(&mut limited, &one),
        Err(BuildError::NodeLimit { limit: 1 })
    );
}
