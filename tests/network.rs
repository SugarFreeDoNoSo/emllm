use emllm::eval::evaluate;
use emllm::expr::{Arena, Node};
use emllm::network::{Network221, NetworkError};
use emllm::program::compile;
use emllm::synth::Rational;

#[path = "../examples/support/reference.rs"]
mod reference;

fn inputs() -> [[Rational; 2]; 3] {
    let zero = Rational::new(0, 1).unwrap();
    let half = Rational::new(1, 2).unwrap();
    let negative = Rational::new(-1, 2).unwrap();
    [[zero, zero], [half, negative], [negative, half]]
}

#[test]
fn validates_input_dimensions_before_constructing_any_nodes() {
    let network = Network221::fixture();
    let mut arena = Arena::new(0);
    let zero = Rational::new(0, 1).unwrap();
    for input in [&[][..], &[zero][..], &[zero; 3][..]] {
        assert_eq!(
            network.build(&mut arena, input),
            Err(NetworkError::InputDimension {
                expected: 2,
                actual: input.len(),
            })
        );
        assert!(arena.nodes().is_empty());
    }
}

#[test]
fn constructs_all_fixture_roots_without_evaluation() {
    let network = Network221::fixture();
    let mut arena = Arena::new(100_000);
    let roots: Vec<_> = inputs()
        .iter()
        .map(|input| network.build(&mut arena, input).unwrap())
        .collect();
    let program = compile(&arena, &roots, 100_000).unwrap();
    assert_eq!(program.roots().len(), 3);
    for (index, node) in program.nodes().iter().enumerate() {
        match node {
            Node::One => {}
            Node::Magma(left, right) => assert!(*left < index && *right < index),
        }
    }
}

#[test]
fn all_three_fixtures_match_the_independent_reference() {
    let network = Network221::fixture();
    for (input, reference_input) in inputs().iter().zip(reference::INPUTS) {
        let mut arena = Arena::new(100_000);
        let root = network.build(&mut arena, input).unwrap();
        let program = compile(&arena, &[root], 100_000).unwrap();
        let actual = evaluate(&program, 100_000).unwrap()[0];
        let expected = reference::infer(reference_input);
        let error = (actual - expected).abs();
        eprintln!(
            "input={reference_input:?}, actual={actual:.15}, reference={expected:.15}, abs_error={error:.3e}, nodes={}",
            program.nodes().len()
        );
        assert!(error <= 1e-6, "input={reference_input:?}, error={error}");
        assert_eq!(
            program
                .nodes()
                .iter()
                .filter(|node| **node == Node::One)
                .count(),
            1
        );
        for (index, node) in program.nodes().iter().enumerate() {
            if let Node::Magma(left, right) = node {
                assert!(*left < index && *right < index);
            }
        }
    }
}

#[test]
fn new_inputs_preserve_previous_closed_programs() {
    let network = Network221::fixture();
    let inputs = inputs();
    let mut arena = Arena::new(100_000);
    let first_root = network.build(&mut arena, &inputs[0]).unwrap();
    let first = compile(&arena, std::slice::from_ref(&first_root), 100_000).unwrap();
    let original_nodes = first.nodes().to_vec();
    let second_root = network.build(&mut arena, &inputs[1]).unwrap();
    assert_ne!(first_root, second_root);
    let second = compile(&arena, &[second_root], 100_000).unwrap();
    let first_again = compile(&arena, &[first_root], 100_000).unwrap();
    assert_eq!(first.nodes(), original_nodes);
    assert_eq!(first_again.nodes(), original_nodes);
    assert_eq!(first_again.roots(), first.roots());
    drop(arena);
    for program in [first, first_again] {
        let actual = evaluate(&program, 100_000).unwrap()[0];
        assert!((actual - reference::infer(reference::INPUTS[0])).abs() <= 1e-6);
    }
    let actual = evaluate(&second, 100_000).unwrap()[0];
    assert!((actual - reference::infer(reference::INPUTS[1])).abs() <= 1e-6);
}
