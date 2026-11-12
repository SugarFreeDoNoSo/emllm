use emllm::eval::evaluate;
use emllm::expr::Arena;
use emllm::program::compile;

#[test]
fn evaluates_elementary_and_shared_expressions() {
    let mut arena = Arena::new(10);
    let one = arena.one().unwrap();
    let e = arena.magma(&one, &one).unwrap();
    let combined = arena.magma(&e, &e).unwrap();
    let program = compile(&arena, &[one, e.clone(), combined, e], 10).unwrap();
    assert_eq!(program.nodes().len(), 3);
    let values = evaluate(&program);
    assert_eq!(values[0], 1.0);
    assert!((values[1] - std::f64::consts::E).abs() <= 1e-12);
    assert!((values[2] - (std::f64::consts::E.exp() - 1.0)).abs() <= 1e-12);
    assert_eq!(values[1], values[3]);
}
