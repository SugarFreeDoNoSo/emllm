//! Verificador externo del corpus escalar: el oráculo nunca entra al constructor.
use std::{error::Error, fs::File, io::Read, time::Instant};

use emllm::{
    eval::evaluate,
    execution::{PlanLimits, plan, run},
    expr::{Arena, Node},
    program::compile,
    synth::{
        model,
        scalar::{F32Bits, synthesize},
    },
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_NODES: usize = 100_000;
const MAX_SLOTS: usize = 100_000;

struct CaseStats {
    structure: [usize; 3],
    absolute_error: f64,
    threshold_fraction: f64,
    differential_fraction: f64,
}

fn check(operation: &str, operands: &[F32Bits], expected: f64) -> Result<CaseStats, String> {
    let mut arena = Arena::new(MAX_NODES);
    let inputs = operands
        .iter()
        .map(|&bits| synthesize(&mut arena, bits))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("construction_scalar: {e:?}"))?;
    let x = &inputs[0];
    let root = match operation {
        "add" => model::add(&mut arena, x, &inputs[1]),
        "sub" => model::sub(&mut arena, x, &inputs[1]),
        "mul" => model::mul(&mut arena, x, &inputs[1]),
        "neg" => model::neg(&mut arena, x),
        "reciprocal" => model::reciprocal_positive(&mut arena, x),
        "rsqrt" => model::inverse_sqrt(&mut arena, x),
        "exp" => model::exp(&mut arena, x),
        "silu" => model::silu(&mut arena, x),
        _ => return Err("unknown_operation".to_owned()),
    }
    .map_err(|e| format!("construction_operation: {e:?}"))?;
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
            "evaluation: {error:?}; V={}; D={}; slots={}; node={:?}",
            program.node_count(),
            program.stats().depth,
            p.stats().slots,
            program.node(error.node.unwrap())
        ));
    }
    let actual = original.unwrap()[0];
    let optimized = planned.map_err(|e| format!("planned_evaluation: {e:?}"))?[0];
    let bound = 1e-6 + 1e-5 * expected.abs();
    if !actual.is_finite() || (actual - expected).abs() > bound {
        return Err(format!(
            "tolerance: actual={actual:.17e}, expected={expected:.17e}, bound={bound:.17e}, root={}, V={}, D={}, slots={}",
            program.roots()[0],
            program.node_count(),
            program.stats().depth,
            p.stats().slots
        ));
    }
    if !optimized.is_finite() || (optimized - actual).abs() > 1e-12 + 1e-12 * actual.abs() {
        return Err(format!(
            "differential: original={actual}, planned={optimized}"
        ));
    }
    Ok(CaseStats {
        structure: [program.node_count(), program.stats().depth, p.stats().slots],
        absolute_error: (actual - expected).abs(),
        threshold_fraction: (actual - expected).abs() / bound,
        differential_fraction: (optimized - actual).abs() / (1e-12 + 1e-12 * actual.abs()),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: verify_operations PATH")?;
    if args.next().is_some() {
        return Err("unexpected arguments".into());
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err("load byte limit".into());
    }
    let text = std::str::from_utf8(&bytes)?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or("missing header")?
        .split_whitespace()
        .collect::<Vec<_>>();
    if header.len() != 2 || header[0] != "EMLOPS1" {
        return Err("invalid header".into());
    }
    let total = header[1].parse::<usize>()?;
    let cases = lines
        .map(|line| -> Result<_, Box<dyn Error>> {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() != 4 {
                return Err("invalid case fields".into());
            }
            let arity = match fields[0] {
                "add" | "sub" | "mul" => 2,
                "neg" | "reciprocal" | "rsqrt" | "exp" | "silu" => 1,
                _ => return Err("unknown operation".into()),
            };
            let mut operands = Vec::new();
            for field in &fields[1..=arity] {
                operands
                    .push(F32Bits::new(u32::from_str_radix(field, 16)?).ok_or("nonfinite input")?);
            }
            if arity == 1 && fields[2] != "-" {
                return Err("unexpected second operand".into());
            }
            let expected = fields[3].parse::<f64>()?;
            if !expected.is_finite() {
                return Err("nonfinite oracle".into());
            }
            Ok((fields[0], operands, expected))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if total == 0 || total != cases.len() {
        return Err("incorrect case count".into());
    }
    let start = Instant::now();
    let mut maxima = [0; 3];
    let mut max_absolute_error = 0.0_f64;
    let mut max_threshold_fraction = 0.0_f64;
    let mut max_differential_fraction = 0.0_f64;
    for (index, (operation, operands, expected)) in cases.iter().enumerate() {
        match check(operation, operands, *expected) {
            Ok(stats) => {
                for (max, value) in maxima.iter_mut().zip(stats.structure) {
                    *max = (*max).max(value);
                }
                max_absolute_error = max_absolute_error.max(stats.absolute_error);
                max_threshold_fraction = max_threshold_fraction.max(stats.threshold_fraction);
                max_differential_fraction =
                    max_differential_fraction.max(stats.differential_fraction);
            }
            Err(message) => {
                let bits = operands.iter().map(|v| v.bits()).collect::<Vec<_>>();
                println!(
                    "{{\"status\":\"failed\",\"index\":{index},\"operation\":{operation:?},\"bits\":{bits:?},\"expected\":{expected},\"total\":{total},\"message\":{message:?},\"elapsed_seconds\":{}}}",
                    start.elapsed().as_secs_f64()
                );
                return Err("operation viability gate failed".into());
            }
        }
    }
    println!(
        "{{\"status\":\"ok\",\"total\":{total},\"max_nodes\":{},\"max_depth\":{},\"max_slots\":{},\"max_absolute_error\":{max_absolute_error},\"max_threshold_fraction\":{max_threshold_fraction},\"max_differential_fraction\":{max_differential_fraction},\"elapsed_seconds\":{}}}",
        maxima[0],
        maxima[1],
        maxima[2],
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
