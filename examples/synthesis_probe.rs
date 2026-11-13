//! Contraejemplo ejecutado: la suma histórica de 7 y 0 desborda internamente.
use std::error::Error;

use emllm::{
    eval::{EvalErrorKind, evaluate},
    execution::{PlanLimits, plan, run},
    expr::Arena,
    program::compile,
    synth::{add, zero},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arena = Arena::new(10_000);
    let one = arena.one()?;
    let mut seven = one.clone();
    // Orientación con primer operando fijo One: todos estos pasos son finitos.
    for _ in 0..6 {
        seven = add(&mut arena, &one, &seven)?;
    }
    let zero = zero(&mut arena)?;
    let bad = add(&mut arena, &seven, &zero)?;
    let inputs = compile(&arena, &[seven, zero], 10_000)?;
    let program = compile(&arena, &[bad], 10_000)?;
    drop(arena);
    let values = evaluate(&inputs, 10_000)?;
    assert!((values[0] - 7.0).abs() <= 1e-8 && values[1].abs() <= 1e-9);
    let plan = plan(
        &program,
        PlanLimits {
            max_nodes: 10_000,
            max_slots: 10_000,
        },
    )?;
    let error = evaluate(&program, 10_000).unwrap_err();
    assert_eq!(error.kind, EvalErrorKind::ExponentialOverflow);
    assert_eq!(run(&plan, 10_000).unwrap_err(), error);
    println!(
        "inputs={values:?} stats={:?} slots={}",
        program.stats(),
        plan.stats().slots
    );
    println!(
        "error={error:?} node={:?} roots={:?}",
        program.node(error.node.unwrap()),
        program.roots()
    );
    if std::env::args().skip(1).any(|arg| arg == "--dump") {
        for (index, node) in program.nodes().enumerate() {
            println!("{index}: {node:?}");
        }
    }
    Ok(())
}
