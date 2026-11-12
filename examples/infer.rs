use std::error::Error;

use emllm::eval::evaluate;
use emllm::expr::Arena;
use emllm::network::Network221;
use emllm::program::compile;
use emllm::synth::Rational;

#[path = "support/reference.rs"]
mod reference;

fn main() -> Result<(), Box<dyn Error>> {
    let build_budget = 100_000;
    let compile_budget = 100_000;
    let evaluation_budget = 100_000;
    let tolerance = 1e-6;
    let mut arena = Arena::new(build_budget);
    let network = Network221::fixture();
    let zero = Rational::new(0, 1)?;
    let half = Rational::new(1, 2)?;
    let negative = Rational::new(-1, 2)?;
    let inputs = [[zero, zero], [half, negative], [negative, half]];

    // Completar todos los programas antes de iniciar la evaluación numérica.
    let mut programs = Vec::new();
    for input in inputs {
        let root = network.build(&mut arena, &input)?;
        programs.push(compile(&arena, &[root], compile_budget)?);
    }
    println!(
        "budgets (nodes): build={build_budget}, compile={compile_budget}, evaluation={evaluation_budget}; arena_nodes={}",
        arena.nodes().len()
    );
    println!("per-program graph: V=unique_nodes, E=operand_links, R=root_positions, D=depth_in_links");
    for (program, input) in programs.iter().zip(reference::INPUTS) {
        let stats = program.stats();
        let actual = evaluate(program, evaluation_budget)?[0];
        let expected = reference::infer(input);
        let error = (actual - expected).abs();
        println!(
            "input={input:?}, V={}, E={}, R={}, D={}, output={actual:.15}, reference={expected:.15}, abs_error={error:.3e}, tolerance={tolerance:.1e}",
            stats.nodes, stats.edges, stats.roots, stats.depth
        );
        if !error.is_finite() || error > tolerance {
            return Err(format!("fixture {input:?} exceeds numerical tolerance: {error}").into());
        }
    }
    Ok(())
}
