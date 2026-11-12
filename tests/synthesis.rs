use emllm::eval::evaluate;
use emllm::expr::{Arena, BuildError, Node};
use emllm::program::compile;
use emllm::synth::{Rational, RationalError, add, mul, neg, rational};

#[test]
fn synthesizes_every_rational_in_the_minimum_domain() {
    let mut arena = Arena::new(10_000);
    let mut max_error = 0.0_f64;
    let mut max_nodes = 0;
    for numerator in -2..=2 {
        for denominator in 1..=2 {
            let value = Rational::new(numerator, denominator).unwrap();
            let root = rational(&mut arena, value).unwrap();
            let program = compile(&arena, &[root], 10_000).unwrap();
            let actual = evaluate(&program, 10_000).unwrap()[0];
            let expected = f64::from(numerator) / f64::from(denominator);
            max_error = max_error.max((actual - expected).abs());
            max_nodes = max_nodes.max(program.nodes().len());
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
    eprintln!("rational max_abs_error={max_error:.3e}, max_reachable_nodes={max_nodes}");
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

#[test]
fn arithmetic_and_compositions_match_an_independent_reference() {
    let mut arena = Arena::new(100_000);
    let mut max_error = 0.0_f64;
    let mut max_nodes = 0;
    for xp in -1..=1 {
        for yp in -1..=1 {
            let x = f64::from(xp) / 2.0;
            let y = f64::from(yp) / 2.0;
            let xr = rational(&mut arena, Rational::new(xp, 2).unwrap()).unwrap();
            let yr = rational(&mut arena, Rational::new(yp, 2).unwrap()).unwrap();
            let sum = add(&mut arena, &xr, &yr).unwrap();
            let product = mul(&mut arena, &xr, &yr).unwrap();
            let negative = neg(&mut arena, &xr).unwrap();
            let composed_sum = add(&mut arena, &product, &negative).unwrap();
            let composed_product = mul(&mut arena, &xr, &sum).unwrap();
            let roots = [sum, product, negative, composed_sum, composed_product];
            let program = compile(&arena, &roots, 100_000).unwrap();
            let actual = evaluate(&program, 100_000).unwrap();
            let expected = [x + y, x * y, -x, x * y - x, x * (x + y)];
            for (operation, (&value, reference)) in actual.iter().zip(expected).enumerate() {
                let error = (value - reference).abs();
                max_error = max_error.max(error);
                assert!(
                    error <= 1e-8,
                    "x={x}, y={y}, operation={operation}, value={value}, expected={reference}, error={error}"
                );
            }
            max_nodes = max_nodes.max(program.nodes().len());
            for (index, node) in program.nodes().iter().enumerate() {
                match node {
                    Node::One => {}
                    Node::Magma(left, right) => assert!(*left < index && *right < index),
                }
            }
        }
    }
    eprintln!("arithmetic max_abs_error={max_error:.3e}, max_reachable_nodes={max_nodes}");
}
