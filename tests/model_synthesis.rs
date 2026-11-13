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
