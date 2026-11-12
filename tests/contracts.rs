use emllm::expr::{Arena, BuildError};
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
