//! Corpus de componentes: preparación por bits, composición y lectura externa.
use emllm::{
    model_io::F32Bits,
    native::{
        CompositionBuilder, NativeLimits, NativeOutput, PreparedBase, decode_with_events,
        tensor::{Shape, TensorView},
    },
};
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    sync::Arc,
    time::Instant,
};
#[path = "support/components.rs"]
mod cases;
use cases::{Case, Input, parse};

fn event(event: &str, phase: &str) -> io::Result<()> {
    println!("EML_NATIVE_PHASE {{\"schema_version\":1,\"event\":{event:?},\"phase\":{phase:?}}}");
    io::stdout().flush()
}

fn prepare(input: &Input, limits: NativeLimits) -> Result<Arc<PreparedBase>, Box<dyn Error>> {
    let mut bits = input.left.clone();
    bits.extend_from_slice(&input.right);
    bits.extend(input.epsilon);
    // Metadatos de las expansiones, incluida la traslación 64 de inverse_sqrt.
    bits.extend(
        [
            0, 0x3f800000, 0xbf800000, 0x40000000, 0x43000000, 0x3c800000, 0x3e000000, 0x45800000,
            0x44000000, 0x3f000000, 0x42800000,
        ]
        .map(|b| F32Bits::new(b).unwrap()),
    );
    Ok(Arc::new(PreparedBase::from_constants(&bits, limits)?))
}

fn compose(
    base: Arc<PreparedBase>,
    input: &Input,
    limits: NativeLimits,
) -> Result<NativeOutput, Box<dyn Error>> {
    let mut builder = CompositionBuilder::new(base.clone(), limits)?;
    let import = |bits| {
        builder
            .base_reference(&base.constant(bits).ok_or("unprepared input")?)
            .map_err(Into::into)
    };
    let left: Vec<_> = input
        .left
        .iter()
        .copied()
        .map(import)
        .collect::<Result<_, Box<dyn Error>>>()?;
    let right: Vec<_> = input
        .right
        .iter()
        .copied()
        .map(import)
        .collect::<Result<_, Box<dyn Error>>>()?;
    let shape = Shape::new(&[1, left.len()])?;
    let roots = match input.operation.as_str() {
        "dot" => vec![builder.dot(&left, &right)?],
        "rmsnorm" => {
            let epsilon = import(input.epsilon.ok_or("missing epsilon")?)?;
            let scale = import(F32Bits::new(0x3c800000).unwrap())?;
            builder
                .rmsnorm(&TensorView::new(shape, &left)?, &right, &epsilon, &scale)?
                .output
                .into_values()
        }
        "swiglu" => builder
            .swiglu(
                &TensorView::new(shape, &left)?,
                &TensorView::new(shape, &right)?,
            )?
            .gated
            .into_values(),
        "softmax" => builder.softmax(&left)?.probabilities,
        _ => return Err("unsupported native component".into()),
    };
    Ok(builder.finish(&roots)?)
}

fn check(index: usize, case: &Case, limits: NativeLimits) -> Result<(), Box<dyn Error>> {
    event("start", "base")?;
    let base = prepare(&case.input, limits)?;
    event("end", "base")?;
    event("start", "compose")?;
    let start = Instant::now();
    let output = compose(base, &case.input, limits)?;
    let composition_seconds = start.elapsed().as_secs_f64();
    event("end", "compose")?;
    let values = decode_with_events(&output, limits, |phase, start| {
        event(if start { "start" } else { "end" }, phase.as_str()).expect("phase output");
    })?;
    event("start", "compare")?;
    if values.original.len() != case.expected.len() {
        return Err("root count mismatch".into());
    }
    let mut fraction = 0.0f64;
    let mut differential = 0.0f64;
    for (root, ((&a, &b), &expected)) in values
        .original
        .iter()
        .zip(&values.planned)
        .zip(&case.expected)
        .enumerate()
    {
        let ratio = (a - expected).abs().max((b - expected).abs()) / (1e-6 + 1e-5 * expected.abs());
        let difference = (a - b).abs() / (1e-12 + 1e-12 * a.abs());
        if !ratio.is_finite() || ratio > 1.0 || difference > 1.0 {
            return Err(format!(
                "root {root}: original={a} planned={b} expected={expected} ratio={ratio}"
            )
            .into());
        }
        fraction = fraction.max(ratio);
        differential = differential.max(difference);
    }
    event("end", "compare")?;
    println!(
        "{{\"event\":\"verified\",\"index\":{index},\"operation\":{:?},\"max_threshold_fraction\":{fraction},\"max_differential_fraction\":{differential},\"base_nodes\":{},\"new_nodes\":{},\"reachable_nodes\":{},\"slots\":{},\"composition_seconds\":{composition_seconds},\"compile_seconds\":{},\"plan_seconds\":{},\"original_seconds\":{},\"planned_seconds\":{}}}",
        case.input.operation,
        output.base().program().node_count(),
        output.delta_pairs().len(),
        values.stats.compilation.reachable_nodes,
        values.stats.plan.slots,
        values.stats.compile_seconds,
        values.stats.plan_seconds,
        values.stats.original_seconds,
        values.stats.planned_seconds
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: verify_native_components PATH")?;
    let mut data = String::new();
    File::open(path)?
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut data)?;
    if data.len() > 8 * 1024 * 1024 {
        return Err("input bytes limit".into());
    }
    let cases = parse(&data)?;
    let limits = NativeLimits {
        base_nodes: 1_000_000,
        closed_nodes: 1_000_000,
        reachable_nodes: 1_000_000,
        numeric_slots: 100_000,
        ..NativeLimits::default()
    };
    for (index, case) in cases.iter().enumerate() {
        if let Err(error) = check(index, case, limits) {
            println!(
                "{{\"event\":\"failed\",\"index\":{index},\"error\":{:?}}}",
                error.to_string()
            );
            return Err(error);
        }
    }
    println!("{{\"event\":\"complete\",\"total\":{}}}", cases.len());
    Ok(())
}
