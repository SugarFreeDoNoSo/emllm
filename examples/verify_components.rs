//! Puerta externa de composiciones: un DAG cerrado por caso, oráculo separado.
use std::{collections::HashMap, error::Error, fs::File, io::Read, time::Instant};

use emllm::{
    eval::evaluate_with_storage,
    execution::{PlanLimits, plan, run_with_storage},
    expr::{Arena, BuildError, Node, NodeId},
    program::compile,
    synth::{
        model,
        scalar::{F32Bits, synthesize},
    },
};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_NODES: usize = 1_000_000;
const MAX_SLOTS: usize = 100_000;

struct Input {
    operation: String,
    left: Vec<F32Bits>,
    right: Vec<F32Bits>,
    epsilon: Option<F32Bits>,
}

struct Case {
    input: Input,
    expected: Vec<f64>,
}

fn bits(text: &str) -> Result<F32Bits, Box<dyn Error>> {
    F32Bits::new(u32::from_str_radix(text, 16)?).ok_or_else(|| "nonfinite input".into())
}

fn parse(text: &str) -> Result<Vec<Case>, Box<dyn Error>> {
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or("missing header")?
        .split_whitespace()
        .collect::<Vec<_>>();
    if header.len() != 2 || header[0] != "EMLCOMP1" {
        return Err("invalid header".into());
    }
    let total = header[1].parse::<usize>()?;
    if total == 0 || total > 10_000 {
        return Err("case count limit".into());
    }
    let mut cases = Vec::new();
    for _ in 0..total {
        let fields = lines
            .next()
            .ok_or("missing case")?
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err("invalid case header".into());
        }
        let length = fields[1].parse::<usize>()?;
        if length == 0 || length > 512 {
            return Err("vector length limit".into());
        }
        let epsilon = match fields[0] {
            "rmsnorm" if length == 64 => Some(bits(fields[2])?),
            "softmax" if fields[2] == "0" => None,
            "dot" | "swiglu" if fields[2] == "-" => None,
            _ => return Err("invalid operation, dimension or auxiliary field".into()),
        };
        let left = lines
            .next()
            .ok_or("missing left vector")?
            .split_whitespace()
            .map(bits)
            .collect::<Result<Vec<_>, _>>()?;
        let right_line = lines.next().ok_or("missing right vector")?;
        let right = if fields[0] == "softmax" {
            if right_line != "-" {
                return Err("unexpected right vector".into());
            }
            Vec::new()
        } else {
            right_line
                .split_whitespace()
                .map(bits)
                .collect::<Result<Vec<_>, _>>()?
        };
        let expected = lines
            .next()
            .ok_or("missing oracle")?
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()?;
        if left.len() != length
            || (fields[0] != "softmax" && right.len() != length)
            || expected.len() != if fields[0] == "dot" { 1 } else { length }
            || expected.iter().any(|x| !x.is_finite())
        {
            return Err("invalid vector or oracle shape/content".into());
        }
        cases.push(Case {
            input: Input {
                operation: fields[0].to_owned(),
                left,
                right,
                epsilon,
            },
            expected,
        });
    }
    if lines.next().is_some() {
        return Err("trailing data".into());
    }
    Ok(cases)
}

fn balanced_sum(arena: &mut Arena, mut values: Vec<NodeId>) -> Result<NodeId, BuildError> {
    while values.len() > 1 {
        values = values
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    model::add(arena, &pair[0], &pair[1])
                } else {
                    Ok(pair[0].clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(values.pop().expect("validated nonempty vector"))
}

fn build(arena: &mut Arena, input: &Input) -> Result<Vec<NodeId>, BuildError> {
    let mut cache = HashMap::<u32, NodeId>::new();
    let mut scalar = |value: F32Bits| -> Result<NodeId, BuildError> {
        if let Some(node) = cache.get(&value.bits()) {
            return Ok(node.clone());
        }
        let node = synthesize(arena, value)?;
        cache.insert(value.bits(), node.clone());
        Ok(node)
    };
    let left = input
        .left
        .iter()
        .copied()
        .map(&mut scalar)
        .collect::<Result<Vec<_>, _>>()?;
    let right = input
        .right
        .iter()
        .copied()
        .map(&mut scalar)
        .collect::<Result<Vec<_>, _>>()?;
    match input.operation.as_str() {
        "dot" => {
            let products = left
                .iter()
                .zip(&right)
                .map(|(a, b)| model::mul(arena, a, b))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(vec![balanced_sum(arena, products)?])
        }
        "rmsnorm" => {
            let squares = left
                .iter()
                .map(|x| model::mul(arena, x, x))
                .collect::<Result<Vec<_>, _>>()?;
            let total = balanced_sum(arena, squares)?;
            let scale = synthesize(arena, F32Bits::new(0x3c80_0000).expect("fixed 1/64"))?;
            let mean = model::mul(arena, &total, &scale)?;
            let epsilon = synthesize(arena, input.epsilon.expect("validated epsilon"))?;
            let denominator = model::add(arena, &mean, &epsilon)?;
            let inverse = model::inverse_sqrt(arena, &denominator)?;
            left.iter()
                .zip(&right)
                .map(|(x, w)| {
                    let normalized = model::mul(arena, x, &inverse)?;
                    model::mul(arena, &normalized, w)
                })
                .collect()
        }
        "softmax" => {
            let exponentials = left
                .iter()
                .map(|x| {
                    let shift = model::sub(arena, x, &left[0])?;
                    model::exp(arena, &shift)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let total = balanced_sum(arena, exponentials.clone())?;
            let inverse = model::reciprocal_positive(arena, &total)?;
            exponentials
                .iter()
                .map(|x| model::mul(arena, x, &inverse))
                .collect()
        }
        "swiglu" => left
            .iter()
            .zip(&right)
            .map(|(gate, up)| {
                let activated = model::silu(arena, gate)?;
                model::mul(arena, &activated, up)
            })
            .collect(),
        _ => unreachable!("validated operation"),
    }
}

fn check(index: usize, case: &Case) -> Result<(), String> {
    let start = Instant::now();
    let mut arena = Arena::new(MAX_NODES);
    let roots = build(&mut arena, &case.input).map_err(|e| format!("construction: {e:?}"))?;
    let build_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let program = compile(&arena, &roots, MAX_NODES).map_err(|e| format!("compilation: {e:?}"))?;
    let compile_seconds = start.elapsed().as_secs_f64();
    drop(arena);
    for (i, node) in program.nodes().enumerate() {
        match node {
            Node::One => assert_eq!(i, 0),
            Node::Magma(a, b) => assert!(a < i && b < i),
        }
    }
    let start = Instant::now();
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: MAX_NODES,
            max_slots: MAX_SLOTS,
        },
    )
    .map_err(|e| format!("planning: {e:?}"))?;
    let plan_seconds = start.elapsed().as_secs_f64();
    let s = program.storage();
    let ps = p.stats();
    let g = program.stats();
    println!(
        "{{\"event\":\"prepared\",\"index\":{index},\"operation\":{:?},\"length\":{},\"nodes\":{},\"edges\":{},\"roots\":{},\"depth\":{},\"slots\":{},\"program_pair_payload_bytes\":{},\"program_pair_capacity_bytes\":{},\"program_root_payload_bytes\":{},\"program_root_capacity_bytes\":{},\"program_metadata_bytes\":{},\"plan_instruction_payload_bytes\":{},\"plan_instruction_capacity_bytes\":{},\"plan_root_payload_bytes\":{},\"plan_root_capacity_bytes\":{},\"plan_metadata_bytes\":{},\"required_original_value_bytes\":{},\"required_planned_value_bytes\":{},\"build_seconds\":{build_seconds},\"compile_seconds\":{compile_seconds},\"plan_seconds\":{plan_seconds}}}",
        case.input.operation,
        case.input.left.len(),
        g.nodes,
        g.edges,
        g.roots,
        g.depth,
        ps.slots,
        s.pair_payload_bytes,
        s.pair_capacity_bytes,
        s.root_payload_bytes,
        s.root_capacity_bytes,
        s.metadata_bytes,
        ps.instruction_payload_bytes,
        ps.instruction_capacity_bytes,
        ps.root_payload_bytes,
        ps.root_capacity_bytes,
        ps.metadata_bytes,
        g.nodes * size_of::<f64>(),
        ps.value_bytes
    );
    let start = Instant::now();
    let original = evaluate_with_storage(&program, MAX_NODES);
    let original_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let planned = run_with_storage(&p, MAX_NODES);
    let planned_seconds = start.elapsed().as_secs_f64();
    let (original, old) = match original {
        Ok(value) => value,
        Err(error) => {
            assert_eq!(planned.unwrap_err(), error);
            return Err(format!(
                "evaluation: {error:?}; expression={:?}",
                error.node.and_then(|node| program.node(node))
            ));
        }
    };
    let (planned, new) = planned.map_err(|e| format!("planned_evaluation: {e:?}"))?;
    assert_eq!(original.len(), case.expected.len());
    let mut max_fraction = 0.0_f64;
    let mut max_differential_fraction = 0.0_f64;
    for (coordinate, ((actual, optimized), reference)) in original
        .iter()
        .zip(&planned)
        .zip(&case.expected)
        .enumerate()
    {
        let bound = 1e-6 + 1e-5 * reference.abs();
        let fraction = (actual - reference).abs() / bound;
        if !actual.is_finite() || fraction > 1.0 {
            return Err(format!(
                "tolerance: coordinate={coordinate}, actual={actual:.17e}, expected={reference:.17e}, bound={bound:.17e}, root={}",
                program.roots()[coordinate]
            ));
        }
        let differential = (optimized - actual).abs() / (1e-12 + 1e-12 * actual.abs());
        if !optimized.is_finite() || differential > 1.0 {
            return Err(format!(
                "differential: coordinate={coordinate}, original={actual}, planned={optimized}"
            ));
        }
        max_fraction = max_fraction.max(fraction);
        max_differential_fraction = max_differential_fraction.max(differential);
    }
    println!(
        "{{\"event\":\"verified\",\"index\":{index},\"max_threshold_fraction\":{max_fraction},\"max_differential_fraction\":{max_differential_fraction},\"original_value_capacity_bytes\":{},\"planned_value_capacity_bytes\":{},\"original_output_capacity_bytes\":{},\"planned_output_capacity_bytes\":{},\"original_buffer_metadata_bytes\":{},\"planned_buffer_metadata_bytes\":{},\"original_seconds\":{original_seconds},\"planned_seconds\":{planned_seconds}}}",
        old.value_capacity_bytes,
        new.value_capacity_bytes,
        old.output_capacity_bytes,
        new.output_capacity_bytes,
        old.buffer_metadata_bytes,
        new.buffer_metadata_bytes
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: verify_components PATH")?;
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
    let cases = parse(std::str::from_utf8(&bytes)?)?;
    for (index, case) in cases.iter().enumerate() {
        if let Err(message) = check(index, case) {
            println!(
                "{{\"event\":\"failed\",\"index\":{index},\"total\":{},\"message\":{message:?}}}",
                cases.len()
            );
            return Err("component viability gate failed".into());
        }
    }
    println!("{{\"event\":\"complete\",\"total\":{}}}", cases.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositions_match_independent_oracles_in_one_closed_dag() {
        let f = |x: f32| F32Bits::new(x.to_bits()).unwrap();
        let cases = [
            Case {
                input: Input {
                    operation: "dot".into(),
                    left: vec![f(0.5), f(-0.5), f(1.0)],
                    right: vec![f(0.5); 3],
                    epsilon: None,
                },
                expected: vec![0.5],
            },
            Case {
                input: Input {
                    operation: "rmsnorm".into(),
                    left: vec![f(0.5); 64],
                    right: vec![f(1.0); 64],
                    epsilon: Some(f(1e-5)),
                },
                expected: vec![0.5 / (0.25 + f64::from(1e-5_f32)).sqrt(); 64],
            },
            Case {
                input: Input {
                    operation: "softmax".into(),
                    left: vec![f(0.5), f(-0.5)],
                    right: vec![],
                    epsilon: None,
                },
                expected: vec![1.0 / (1.0 + (-1.0_f64).exp()), 1.0 / (1.0 + 1.0_f64.exp())],
            },
            Case {
                input: Input {
                    operation: "swiglu".into(),
                    left: vec![f(-0.5)],
                    right: vec![f(1.5)],
                    epsilon: None,
                },
                expected: vec![-0.75 / (1.0 + 0.5_f64.exp())],
            },
        ];
        for (index, case) in cases.iter().enumerate() {
            check(index, case).unwrap();
        }
    }

    #[test]
    fn protocol_rejects_inconsistent_shapes_and_nonfinite_oracles() {
        assert!(parse("EMLCOMP1 1\nsoftmax 2 0\n00000000 00000000\n-\n0.5 0.5\n").is_ok());
        for text in [
            "EMLCOMP1 1\nsoftmax 2 1\n00000000 00000000\n-\n0.5 0.5\n",
            "EMLCOMP1 1\ndot 2 -\n00000000\n00000000 00000000\n0\n",
            "EMLCOMP1 1\ndot 1 -\n00000000\n00000000\nNaN\n",
            "EMLCOMP1 1\nrmsnorm 1 3727c5ac\n00000000\n00000000\n0\n",
        ] {
            assert!(parse(text).is_err());
        }
    }
}
