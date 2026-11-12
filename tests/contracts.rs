use emllm::eval::{EvalError, EvalErrorKind, evaluate};
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

#[test]
fn inspection_resolves_shared_operands_in_their_own_snapshot() {
    let mut arena = Arena::new(4);
    let one = arena.one().unwrap();
    let t = arena.magma(&one, &one).unwrap();
    arena.magma(&one, &t).unwrap(); // Excluido; fuerza remapeo del padre.
    let root = arena.magma(&t, &t).unwrap();
    let program = compile(&arena, std::slice::from_ref(&root), 3).unwrap();
    let compiled_root = program.roots()[0];
    assert_ne!(compiled_root, root.index());
    let Node::Magma(left, right) = *program.node(compiled_root).unwrap() else {
        panic!("expected a shared Magma root");
    };
    assert_eq!(left, right);
    assert_eq!(program.node(left), Some(&Node::Magma(0, 0)));
    for (index, node) in program.nodes().iter().enumerate() {
        assert_eq!(program.node(index), Some(node));
        if let Node::Magma(left, right) = node {
            assert!(*left < index && *right < index);
            assert!(program.node(*left).is_some());
            assert!(program.node(*right).is_some());
        }
    }
    assert!(program.roots().iter().all(|&root| program.node(root).is_some()));
    assert_eq!(program.node(usize::MAX), None);
}

#[test]
fn errors_distinguish_stages_and_never_return_partial_outputs() {
    assert_eq!(Arena::new(0).one(), Err(BuildError::NodeLimit { limit: 0 }));
    let mut arena = Arena::new(6);
    let one = arena.one().unwrap();
    let a = arena.magma(&one, &one).unwrap();
    let b = arena.magma(&a, &one).unwrap();
    let c = arena.magma(&b, &one).unwrap();
    let overflow = arena.magma(&c, &one).unwrap();
    let dependent = arena.magma(&overflow, &one).unwrap();
    let roots = [one.clone(), dependent, one];
    assert!(matches!(
        compile(&arena, &[], 6),
        Err(CompileError::Roots(BuildError::EmptyRoots))
    ));
    assert!(matches!(
        compile(&arena, &roots, 0),
        Err(CompileError::NodeLimit { limit: 0 })
    ));
    let program = compile(&arena, &roots, 6).unwrap();
    assert_eq!(
        evaluate(&program, 0),
        Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: 0 },
            node: None,
        })
    );
    let Node::Magma(failing_node, _) = *program.node(program.roots()[1]).unwrap() else {
        panic!("expected a dependent Magma");
    };
    assert_ne!(failing_node, program.roots()[1]);
    assert!(matches!(program.node(failing_node), Some(Node::Magma(_, _))));
    assert_eq!(
        evaluate(&program, 6),
        Err(EvalError {
            kind: EvalErrorKind::ExponentialOverflow,
            node: Some(failing_node),
        })
    );
}

#[test]
fn interleaved_evaluations_and_budget_retries_preserve_the_program() {
    let mut arena = Arena::new(5);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let good = compile(&arena, &[e.clone(), one.clone(), e.clone()], 2).unwrap();
    let ee = arena.magma(&e, &one).unwrap();
    let large = arena.magma(&ee, &one).unwrap();
    let overflow = arena.magma(&large, &one).unwrap();
    let bad = compile(&arena, &[overflow], 5).unwrap();
    let nodes = good.nodes().to_vec();
    let roots = good.roots().to_vec();
    drop(arena);

    for _ in 0..2 {
        let result = evaluate(&good, 2).unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0] - std::f64::consts::E).abs() <= 1e-12);
        assert_eq!(result[1], 1.0);
        assert!((result[2] - std::f64::consts::E).abs() <= 1e-12);
        assert_eq!(
            evaluate(&bad, 5).unwrap_err().kind,
            EvalErrorKind::ExponentialOverflow
        );
        assert_eq!(
            evaluate(&good, 1).unwrap_err().kind,
            EvalErrorKind::NodeLimit { limit: 1 }
        );
    }
    let retry = evaluate(&good, 2).unwrap();
    assert!((retry[0] - std::f64::consts::E).abs() <= 1e-12);
    assert_eq!(good.nodes(), nodes);
    assert_eq!(good.roots(), roots);
}
