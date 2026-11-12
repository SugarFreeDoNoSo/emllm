use emllm::expr::{Arena, Node};
use emllm::network::{Network221, NetworkError};
use emllm::program::compile;
use emllm::synth::Rational;

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
