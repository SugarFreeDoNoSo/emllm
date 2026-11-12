use emllm::eval::evaluate;
use emllm::expr::{Arena, BuildError, Node};
use emllm::program::compile;
use emllm::synth::{Rational, RationalError, rational};

#[test]
fn synthesizes_every_rational_in_the_minimum_domain() {
    let mut arena = Arena::new(10_000);
    for numerator in -2..=2 {
        for denominator in 1..=2 {
            let value = Rational::new(numerator, denominator).unwrap();
            let root = rational(&mut arena, value).unwrap();
            let program = compile(&arena, &[root], 10_000).unwrap();
            let actual = evaluate(&program, 10_000).unwrap()[0];
            let expected = f64::from(numerator) / f64::from(denominator);
            assert!(
                (actual - expected).abs() <= 1e-9,
                "{numerator}/{denominator}: actual={actual}, expected={expected}"
            );
            for (index, node) in program.nodes().iter().enumerate() {
                match node {
                    Node::One => {}
                    Node::Magma(left, right) => assert!(*left < index && *right < index),
                }
            }
        }
    }
}

#[test]
fn normalizes_before_domain_validation_without_integer_overflow() {
    assert_eq!(Rational::new(2, 2), Rational::new(1, 1));
    assert_eq!(Rational::new(0, u32::MAX), Rational::new(0, 1));
    assert_eq!(Rational::new(i32::MIN, 1_u32 << 31), Rational::new(-1, 1));
    assert_eq!(Rational::new(1, 0), Err(RationalError::ZeroDenominator));
    assert!(matches!(
        Rational::new(1, 3),
        Err(RationalError::OutsideDomain { .. })
    ));
}

#[test]
fn synthesis_respects_construction_budget() {
    let mut arena = Arena::new(2);
    let result = rational(&mut arena, Rational::new(1, 2).unwrap());
    assert_eq!(result, Err(BuildError::NodeLimit { limit: 2 }));
    assert_eq!(arena.nodes().len(), 2);
}
