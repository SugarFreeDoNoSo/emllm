use emllm::eval::evaluate;
use emllm::expr::{Arena, BuildError, Node};
use emllm::program::{CompileError, compile};

#[test]
fn construction_handles_keep_their_provenance_after_the_arena_is_dropped() {
    let mut original = Arena::new(2);
    let foreign = original.one().unwrap();
    let mut receiver = Arena::new(2);
    let local = receiver.one().unwrap();
    assert_eq!(foreign.index(), local.index());
    assert_ne!(foreign, local);
    drop(original);

    for (left, right) in [(&foreign, &local), (&local, &foreign)] {
        assert_eq!(
            receiver.magma(left, right),
            Err(BuildError::ForeignReference)
        );
    }
    assert!(matches!(
        compile(&receiver, std::slice::from_ref(&foreign), 2),
        Err(CompileError::Roots(BuildError::ForeignReference))
    ));
    assert_eq!(receiver.nodes().len(), 1);

    let mut replacement = Arena::new(1);
    let new = replacement.one().unwrap();
    assert_eq!(new.index(), foreign.index());
    assert_ne!(new, foreign);
    assert_eq!(
        replacement.validate_roots(&[foreign]),
        Err(BuildError::ForeignReference)
    );
}

#[test]
fn compilation_preserves_root_positions_and_sharing_within_its_budget() {
    let mut arena = Arena::new(3);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    arena.magma(&t, &one).unwrap(); // No alcanzable desde las raíces seleccionadas.
    let roots = [t.clone(), one, t];
    let program = compile(&arena, &roots, 2).unwrap();
    assert_eq!(program.nodes(), &[Node::One, Node::Magma(0, 0)]);
    assert_eq!(program.roots(), &[1, 0, 1]);
    let repeated = compile(&arena, &roots, 2).unwrap();
    assert_eq!(program.nodes(), repeated.nodes());
    assert_eq!(program.roots(), repeated.roots());
    let values = evaluate(&program, 2).unwrap();
    assert_eq!(values.len(), 3);
    assert!((values[0] - std::f64::consts::E).abs() <= 1e-12);
    assert_eq!(values[1], 1.0);
    assert_eq!(values[0], values[2]); // Dos posiciones del mismo valor de esta llamada.
    assert!(matches!(
        compile(&arena, &roots, 1),
        Err(CompileError::NodeLimit { limit: 1 })
    ));
    assert!(matches!(
        compile(&arena, &[], 3),
        Err(CompileError::Roots(BuildError::EmptyRoots))
    ));
}
