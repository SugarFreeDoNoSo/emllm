//! Payload/capacidad del programa residente y los dos modos de evaluación.
use std::{error::Error, mem::size_of};

use emllm::{
    eval::evaluate_with_storage,
    execution::{PlanLimits, plan, run_with_storage},
    expr::Arena,
    network::Network221,
    program::{Program, compile},
    synth::Rational,
};

#[path = "support/reference.rs"]
mod reference;

fn report(name: &str, program: &Program) -> Result<f64, Box<dyn Error>> {
    let planned = plan(
        program,
        PlanLimits {
            max_nodes: 100_000,
            max_slots: 100_000,
        },
    )?;
    let (original, old) = evaluate_with_storage(program, 100_000)?;
    let (optimized, new) = run_with_storage(&planned, 100_000)?;
    for (&a, &b) in optimized.iter().zip(&original) {
        assert!(a.is_finite() && (a - b).abs() <= 1e-12 + 1e-12 * b.abs());
    }
    let s = program.storage();
    let p = planned.stats();
    assert_eq!(old.values, program.node_count());
    assert_eq!(new.values, p.slots);
    assert_eq!(new.value_payload_bytes, p.value_bytes);
    assert_eq!(p.instruction_bytes, size_of::<[usize; 3]>());
    let resident_payload = s.pair_payload_bytes + s.root_payload_bytes + s.metadata_bytes;
    let resident_capacity = s.pair_capacity_bytes + s.root_capacity_bytes + s.metadata_bytes;
    let original_payload = resident_payload
        + old.value_payload_bytes
        + old.output_payload_bytes
        + old.buffer_metadata_bytes;
    let original_capacity = resident_capacity
        + old.value_capacity_bytes
        + old.output_capacity_bytes
        + old.buffer_metadata_bytes;
    let planned_payload = resident_payload
        + p.instruction_payload_bytes
        + p.root_payload_bytes
        + p.metadata_bytes
        + new.value_payload_bytes
        + new.output_payload_bytes
        + new.buffer_metadata_bytes;
    let planned_capacity = resident_capacity
        + p.instruction_capacity_bytes
        + p.root_capacity_bytes
        + p.metadata_bytes
        + new.value_capacity_bytes
        + new.output_capacity_bytes
        + new.buffer_metadata_bytes;
    println!("case={name} graph={:?}", program.stats());
    println!("program={s:?}\nplan={p:?}\noriginal_buffers={old:?}\nplanned_buffers={new:?}");
    println!(
        "original_total_payload_bytes={original_payload} original_total_capacity_bytes={original_capacity} planned_total_payload_bytes={planned_payload} planned_total_capacity_bytes={planned_capacity}"
    );
    println!(
        "output={:.15} differential_abs={:.3e}",
        optimized[0],
        (optimized[0] - original[0]).abs()
    );
    Ok(optimized[0])
}

fn main() -> Result<(), Box<dyn Error>> {
    println!(
        "bytes: usize={} pair={} f64={}; RSS=unavailable (not measured)",
        size_of::<usize>(),
        size_of::<[usize; 2]>(),
        size_of::<f64>()
    );
    println!(
        "totals include resident Program, optional plan, values, roots, outputs and their inline metadata; exclude allocator, build/compile/plan temporaries and diagnostic records"
    );
    let mut arena = Arena::new(10_001);
    let one = arena.one()?;
    let mut root = one.clone();
    for _ in 0..10_000 {
        root = arena.magma(&one, &root)?;
    }
    let chain = compile(&arena, &[root], 10_001)?;
    drop(arena);
    report("chain10000", &chain)?;
    let mut arena = Arena::new(100_000);
    let network = Network221::fixture();
    let z = Rational::new(0, 1)?;
    let h = Rational::new(1, 2)?;
    let n = Rational::new(-1, 2)?;
    for (index, input) in [[z, z], [h, n], [n, h]].iter().enumerate() {
        let root = network.build(&mut arena, input)?;
        let program = compile(&arena, &[root], 100_000)?;
        let output = report(&format!("network{index}"), &program)?;
        assert!((output - reference::infer(reference::INPUTS[index])).abs() <= 1e-6);
    }
    Ok(())
}
