//! Medición estructural acotada: pesos compartidos y coste marginal de expansiones.
//! No ejecuta los nodos ni construye un Transformer parcial con activaciones externas.
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::File,
    io::Read,
    time::Instant,
};

use emllm::{
    execution::{PlanLimits, plan},
    expr::{Arena, NodeId},
    model_io::{F32Bits, LoadLimits, Stories260K, TensorKind, read_stories260k},
    program::compile,
    synth::{model, scalar::synthesize},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_NODES: usize = 10_000_000;
const MAX_SLOTS: usize = 1_000_000;

fn is_weight(kind: TensorKind) -> bool {
    !matches!(
        kind,
        TensorKind::RopeCos
            | TensorKind::RopeSin
            | TensorKind::Epsilon
            | TensorKind::AttentionScale
    )
}

fn extrema(values: &[usize]) -> (usize, usize) {
    (
        *values.iter().min().expect("nonempty sample"),
        *values.iter().max().expect("nonempty sample"),
    )
}

fn marginal_cost(values: &[F32Bits]) -> Result<(), Box<dyn Error>> {
    let mut arena = Arena::new(1_000_000);
    let inputs = values
        .iter()
        .map(|&x| synthesize(&mut arena, x))
        .collect::<Result<Vec<_>, _>>()?;
    let left = &inputs[..64];
    let right = &inputs[64..];
    let before = arena.node_count();
    model::mul(&mut arena, &left[0], &right[0])?;
    let priming = arena.node_count() - before;
    let mut left_warm = Vec::new();
    let mut right_warm = Vec::new();
    for x in &left[1..] {
        let before = arena.node_count();
        model::mul(&mut arena, x, &right[0])?;
        left_warm.push(arena.node_count() - before);
    }
    for y in &right[1..] {
        let before = arena.node_count();
        model::mul(&mut arena, &left[0], y)?;
        right_warm.push(arena.node_count() - before);
    }
    let mut products = Vec::new();
    let mut mul_costs = Vec::new();
    for x in &left[1..] {
        for y in &right[1..] {
            let before = arena.node_count();
            products.push(model::mul(&mut arena, x, y)?);
            mul_costs.push(arena.node_count() - before);
        }
    }
    let before = arena.node_count();
    let repeated = model::mul(&mut arena, &left[1], &right[1])?;
    assert_eq!(repeated, products[0]);
    assert_eq!(arena.node_count(), before);
    let mut add_costs = Vec::new();
    while products.len() > 1 {
        products = products
            .chunks(2)
            .map(|pair| {
                if pair.len() == 1 {
                    return Ok(pair[0].clone());
                }
                let before = arena.node_count();
                let root = model::add(&mut arena, &pair[0], &pair[1])?;
                add_costs.push(arena.node_count() - before);
                Ok(root)
            })
            .collect::<Result<Vec<_>, emllm::expr::BuildError>>()?;
    }
    let (mul_min, mul_max) = extrema(&mul_costs);
    let (add_min, add_max) = extrema(&add_costs);
    let (left_min, left_max) = extrema(&left_warm);
    let (right_min, right_max) = extrema(&right_warm);
    let sample_bits = values.iter().map(|x| x.bits()).collect::<Vec<_>>();
    println!(
        "{{\"event\":\"marginal\",\"sample_bits\":{sample_bits:?},\"priming_nodes\":{priming},\"products\":{},\"additions\":{},\"mul_min\":{mul_min},\"mul_max\":{mul_max},\"add_min\":{add_min},\"add_max\":{add_max},\"left_warm_min\":{left_min},\"left_warm_max\":{left_max},\"right_warm_min\":{right_min},\"right_warm_max\":{right_max},\"repeated_pair_new_nodes\":0,\"arena_nodes\":{}}}",
        mul_costs.len(),
        add_costs.len(),
        arena.node_count()
    );
    Ok(())
}

fn weights_cost(model: &Stories260K<'_>) -> Result<(), Box<dyn Error>> {
    let mut arena = Arena::new(MAX_NODES);
    let mut cache = HashMap::<u32, NodeId>::new();
    let mut roots = Vec::new();
    let mut scalars = 0;
    let mut tensors = 0;
    let start = Instant::now();
    for tensor in model.package().tensors() {
        if !is_weight(tensor.kind()) || tensor.alias().is_some() {
            continue;
        }
        for (index, value) in tensor.scalars().enumerate() {
            scalars += 1;
            if cache.contains_key(&value.bits()) {
                continue;
            }
            let root = match synthesize(&mut arena, value) {
                Ok(root) => root,
                Err(error) => {
                    let kind = format!("{:?}", tensor.kind());
                    let message = format!("{error:?}");
                    let layer = tensor.layer().map_or("null".to_owned(), |v| v.to_string());
                    println!(
                        "{{\"event\":\"weights_failed\",\"tensor\":{kind:?},\"layer\":{layer},\"index\":{index},\"bits\":{},\"distinct_completed\":{},\"arena_nodes\":{},\"elapsed_seconds\":{},\"message\":{message:?}}}",
                        value.bits(),
                        cache.len(),
                        arena.node_count(),
                        start.elapsed().as_secs_f64()
                    );
                    return Err(error.into());
                }
            };
            cache.insert(value.bits(), root.clone());
            roots.push(root);
        }
        tensors += 1;
    }
    let build_seconds = start.elapsed().as_secs_f64();
    let arena_nodes = arena.node_count();
    drop(cache);
    let start = Instant::now();
    let program = compile(&arena, &roots, MAX_NODES)?;
    let compile_seconds = start.elapsed().as_secs_f64();
    drop(roots);
    drop(arena);
    let start = Instant::now();
    let p = plan(
        &program,
        PlanLimits {
            max_nodes: MAX_NODES,
            max_slots: MAX_SLOTS,
        },
    )?;
    let plan_seconds = start.elapsed().as_secs_f64();
    let g = program.stats();
    let s = program.storage();
    let ps = p.stats();
    let program_capacity = s.pair_capacity_bytes + s.root_capacity_bytes + s.metadata_bytes;
    let planned_capacity = program_capacity
        + ps.instruction_capacity_bytes
        + ps.root_capacity_bytes
        + ps.metadata_bytes;
    println!(
        "{{\"event\":\"weights\",\"tensors\":{tensors},\"scalars\":{scalars},\"distinct\":{},\"arena_nodes\":{arena_nodes},\"nodes\":{},\"edges\":{},\"roots\":{},\"depth\":{},\"slots\":{},\"pair_payload_bytes\":{},\"pair_capacity_bytes\":{},\"instruction_payload_bytes\":{},\"instruction_capacity_bytes\":{},\"program_resident_capacity_bytes\":{program_capacity},\"planned_resident_capacity_bytes\":{planned_capacity},\"required_original_value_bytes\":{},\"required_planned_value_bytes\":{},\"build_seconds\":{build_seconds},\"compile_seconds\":{compile_seconds},\"plan_seconds\":{plan_seconds}}}",
        g.roots,
        g.nodes,
        g.edges,
        g.roots,
        g.depth,
        ps.slots,
        s.pair_payload_bytes,
        s.pair_capacity_bytes,
        ps.instruction_payload_bytes,
        ps.instruction_capacity_bytes,
        g.nodes * size_of::<f64>(),
        ps.value_bytes
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: synthesis_cost MODEL.emltiny")?;
    if args.next().is_some() {
        return Err("unexpected arguments".into());
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let model = read_stories260k(
        &bytes,
        LoadLimits {
            max_bytes: MAX_BYTES,
            max_tensors: 64,
        },
    )?;
    let mut seen = HashSet::new();
    let sample = model
        .package()
        .tensors()
        .iter()
        .filter(|t| is_weight(t.kind()) && t.alias().is_none())
        .flat_map(|t| t.scalars())
        .filter(|v| seen.insert(v.bits()))
        .take(128)
        .collect::<Vec<_>>();
    if sample.len() != 128 {
        return Err("insufficient distinct weights".into());
    }
    marginal_cost(&sample)?;
    weights_cost(&model)
}
