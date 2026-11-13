//! Puerta escalar explícita; consume u32 little-endian o --bits HEX.
use std::{error::Error, fs::File, io::Read, time::Instant};

use emllm::{
    eval::evaluate,
    execution::{PlanLimits, plan, run},
    expr::{Arena, Node},
    program::compile,
    synth::scalar::{F32Bits, synthesize},
};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_NODES: usize = 10_000;
const MAX_SLOTS: usize = 10_000;

struct Metrics {
    nodes: usize,
    depth: usize,
    slots: usize,
    error: f64,
    ratio: f64,
}

fn check(bits: u32) -> Result<Metrics, String> {
    let value = F32Bits::new(bits).ok_or("load: nonfinite bits")?;
    let mut arena = Arena::new(MAX_NODES);
    let root = synthesize(&mut arena, value).map_err(|e| format!("construction: {e:?}"))?;
    let program = compile(&arena, &[root], MAX_NODES).map_err(|e| format!("compilation: {e:?}"))?;
    drop(arena);
    for (i, node) in program.nodes().enumerate() {
        match node {
            Node::One => assert_eq!(i, 0),
            Node::Magma(a, b) => assert!(a < i && b < i),
        }
    }
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: MAX_NODES,
            max_slots: MAX_SLOTS,
        },
    )
    .map_err(|e| format!("planning: {e:?}"))?;
    let original = evaluate(&program, MAX_NODES);
    let planned = run(&p, MAX_NODES);
    if let Err(error) = original {
        assert_eq!(planned.unwrap_err(), error);
        return Err(format!(
            "evaluation: {error:?}; V={}; D={}; slots={}",
            program.node_count(),
            program.stats().depth,
            p.stats().slots
        ));
    }
    let actual = original.unwrap()[0];
    let optimized = planned.map_err(|e| format!("planned_evaluation: {e:?}"))?[0];
    let expected = value.to_f64();
    let error = (actual - expected).abs();
    let bound = 1e-9 + 1e-7 * expected.abs();
    if error > bound || !actual.is_finite() {
        return Err(format!(
            "tolerance: actual={actual:.17e}, expected={expected:.17e}, error={error:.17e}, bound={bound:.17e}, root={}",
            program.roots()[0]
        ));
    }
    if !optimized.is_finite() || (optimized - actual).abs() > 1e-12 + 1e-12 * actual.abs() {
        return Err(format!(
            "differential: original={actual}, planned={optimized}"
        ));
    }
    Ok(Metrics {
        nodes: program.node_count(),
        depth: program.stats().depth,
        slots: p.stats().slots,
        error,
        ratio: error / bound,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: verify_scalars PATH | --bits HEX")?;
    let bits = if input == "--bits" {
        vec![u32::from_str_radix(
            args.next().ok_or("missing bits")?.trim_start_matches("0x"),
            16,
        )?]
    } else {
        let mut bytes = Vec::new();
        File::open(input)?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES || bytes.is_empty() || bytes.len() % 4 != 0 {
            return Err("invalid scalar corpus length or load limit".into());
        }
        bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect()
    };
    if args.next().is_some() {
        return Err("unexpected arguments".into());
    }
    let start = Instant::now();
    let mut maximum = Metrics {
        nodes: 0,
        depth: 0,
        slots: 0,
        error: 0.0,
        ratio: 0.0,
    };
    for (index, &value) in bits.iter().enumerate() {
        match check(value) {
            Ok(m) => {
                maximum.nodes = maximum.nodes.max(m.nodes);
                maximum.depth = maximum.depth.max(m.depth);
                maximum.slots = maximum.slots.max(m.slots);
                maximum.error = maximum.error.max(m.error);
                maximum.ratio = maximum.ratio.max(m.ratio);
            }
            Err(message) => {
                println!(
                    "{{\"status\":\"failed\",\"index\":{index},\"bits\":{value},\"total\":{},\"message\":{message:?},\"elapsed_seconds\":{}}}",
                    bits.len(),
                    start.elapsed().as_secs_f64()
                );
                return Err("scalar viability gate failed".into());
            }
        }
    }
    println!(
        "{{\"status\":\"ok\",\"total\":{},\"max_nodes\":{},\"max_depth\":{},\"max_slots\":{},\"max_abs_error\":{},\"max_tolerance_ratio\":{},\"limits\":{{\"bytes\":{MAX_BYTES},\"nodes\":{MAX_NODES},\"slots\":{MAX_SLOTS}}},\"elapsed_seconds\":{}}}",
        bits.len(),
        maximum.nodes,
        maximum.depth,
        maximum.slots,
        maximum.error,
        maximum.ratio,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
