//! Preparación estructural completa seguida de lectura externa, con eventos de fase.
use emllm::{
    eval::evaluate_with_storage,
    execution::{PlanLimits, plan, run_with_storage},
    model_io::{LoadLimits, read_stories260k},
    native::{NativeLimits, Phase, PreparedBase, SYNTHESIS_VERSION},
};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::PathBuf,
    time::Instant,
};

fn phase(event: &str, phase: Phase) -> io::Result<()> {
    println!(
        "EML_NATIVE_PHASE {{\"schema_version\":1,\"event\":{event:?},\"phase\":{:?}}}",
        phase.as_str()
    );
    io::stdout().flush()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let package = args
        .next()
        .ok_or("usage: prepare_native_base PACKAGE OUTPUT [name=value ...]")?;
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    let mut limits = NativeLimits::default();
    for argument in args {
        let (key, value) = argument.split_once('=').ok_or("expected name=value")?;
        let value: usize = value.parse()?;
        match key {
            "package_bytes" => limits.package_bytes = value,
            "tensors" => limits.tensors = value,
            "base_nodes" => limits.base_nodes = value,
            "template_instructions" => limits.template_instructions = value,
            "ports" => limits.ports = value,
            "roots" => limits.roots = value,
            "closed_nodes" => limits.closed_nodes = value,
            "reachable_nodes" => limits.reachable_nodes = value,
            "numeric_slots" => limits.numeric_slots = value,
            _ => return Err("unknown limit".into()),
        }
    }
    fs::create_dir(&output)?;
    phase("start", Phase::Load)?;
    let mut bytes = Vec::new();
    File::open(package)?
        .take(limits.package_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    let model = read_stories260k(
        &bytes,
        LoadLimits {
            max_bytes: limits.package_bytes,
            max_tensors: limits.tensors,
        },
    )?;
    phase("end", Phase::Load)?;
    phase("start", Phase::Base)?;
    let start = Instant::now();
    let base = match PreparedBase::from_stories260k(&model, limits) {
        Ok(base) => base,
        Err(error) => {
            println!(
                "{{\"event\":\"base-failed\",\"tensor\":{},\"scalar\":{},\"bits\":{},\"completed_constants\":{},\"constructed_nodes\":{},\"error\":{:?}}}",
                optional(error.tensor),
                optional(error.scalar),
                error.bits.map_or("null".into(), |v| v.to_string()),
                error.completed_constants,
                error.constructed_nodes,
                error.to_string()
            );
            return Err(error.into());
        }
    };
    let preparation_seconds = start.elapsed().as_secs_f64();
    drop(model);
    drop(bytes);
    phase("end", Phase::Base)?;
    let s = base.storage();
    println!(
        "{{\"event\":\"base-prepared\",\"synthesis_version\":{SYNTHESIS_VERSION:?},\"preparation_seconds\":{preparation_seconds},\"nodes\":{},\"constants\":{},\"tensor_entries\":{},\"tensors\":{},\"vocabulary_symbols\":{},\"depth\":{},\"vector_payload_bytes\":{},\"vector_capacity_bytes\":{},\"pair_index_len\":{},\"pair_index_capacity\":{},\"pair_index_modeled_payload_bytes\":{},\"constant_index_len\":{},\"constant_index_capacity\":{},\"constant_index_modeled_payload_bytes\":{},\"inline_metadata_bytes\":{}}}",
        s.nodes,
        s.constants,
        s.tensor_entries,
        base.tensors().len(),
        s.vocabulary_symbols,
        base.program().stats().depth,
        s.vector_payload_bytes,
        s.vector_capacity_bytes,
        s.pair_index_len,
        s.pair_index_capacity,
        s.pair_index_modeled_payload_bytes,
        s.constant_index_len,
        s.constant_index_capacity,
        s.constant_index_modeled_payload_bytes,
        s.inline_metadata_bytes
    );
    io::stdout().flush()?;
    phase("start", Phase::Plan)?;
    let start = Instant::now();
    let plan = plan(
        base.program(),
        PlanLimits {
            max_nodes: limits.reachable_nodes,
            max_slots: limits.numeric_slots,
        },
    )?;
    let plan_seconds = start.elapsed().as_secs_f64();
    let ps = plan.stats();
    phase("end", Phase::Plan)?;
    phase("start", Phase::EvaluateOriginal)?;
    let start = Instant::now();
    let (original, os) = evaluate_with_storage(base.program(), limits.reachable_nodes)?;
    let original_seconds = start.elapsed().as_secs_f64();
    phase("end", Phase::EvaluateOriginal)?;
    phase("start", Phase::EvaluatePlanned)?;
    let start = Instant::now();
    let (planned, ns) = run_with_storage(&plan, limits.reachable_nodes)?;
    let planned_seconds = start.elapsed().as_secs_f64();
    phase("end", Phase::EvaluatePlanned)?;
    phase("start", Phase::Compare)?;
    let mut max_fraction = 0.0f64;
    let mut max_differential = 0.0f64;
    for (index, ((&bits, &a), &b)) in base
        .constant_bits()
        .iter()
        .zip(&original)
        .zip(&planned)
        .enumerate()
    {
        let reference = bits.to_f64();
        let bound = 1e-9 + 1e-7 * reference.abs();
        let error = (a - reference).abs().max((b - reference).abs()) / bound;
        let differential = (a - b).abs() / (1e-12 + 1e-12 * a.abs());
        if !a.is_finite() || !b.is_finite() || error > 1.0 || differential > 1.0 {
            return Err(format!("constant {index} bits={:08x} root={} original={a} planned={b} reference={reference}",
                bits.bits(), base.program().roots()[index]).into());
        }
        max_fraction = max_fraction.max(error);
        max_differential = max_differential.max(differential);
    }
    phase("end", Phase::Compare)?;
    phase("start", Phase::Emit)?;
    let mut constants = BufWriter::new(File::create(output.join("constants.tsv"))?);
    for (i, bits) in base.constant_bits().iter().enumerate() {
        writeln!(
            constants,
            "{:08x}\t{}\t{:.17e}\t{:.17e}",
            bits.bits(),
            base.program().roots()[i],
            original[i],
            planned[i]
        )?;
    }
    constants.flush()?;
    let mut tensors = BufWriter::new(File::create(output.join("tensors.tsv"))?);
    for (i, tensor) in base.tensors().iter().enumerate() {
        println!(
            "{{\"event\":\"tensor\",\"index\":{i},\"kind\":{},\"layer\":{},\"shape\":{:?},\"alias\":{},\"scalars\":{}}}",
            tensor.kind() as u32,
            tensor.layer().map_or("null".into(), |n| n.to_string()),
            tensor.shape(),
            optional(tensor.alias()),
            tensor.len()
        );
        for index in 0..tensor.len() {
            let (bits, root) = base.tensor_scalar(i, index).expect("bounded tensor index");
            writeln!(
                tensors,
                "{i}\t{index}\t{:08x}\t{}",
                bits.bits(),
                root.index()
            )?;
        }
    }
    tensors.flush()?;
    let mut vocabulary = BufWriter::new(File::create(output.join("vocabulary.tsv"))?);
    for id in 0..512 {
        writeln!(
            vocabulary,
            "{id}\t{}",
            base.vocabulary_symbol(id).unwrap().index()
        )?;
    }
    vocabulary.flush()?;
    println!(
        "{{\"event\":\"base-verified\",\"constants\":{},\"max_threshold_fraction\":{max_fraction},\"max_differential_fraction\":{max_differential},\"plan_seconds\":{plan_seconds},\"original_seconds\":{original_seconds},\"planned_seconds\":{planned_seconds},\"slots\":{},\"plan_instruction_capacity_bytes\":{},\"plan_root_capacity_bytes\":{},\"original_value_capacity_bytes\":{},\"planned_value_capacity_bytes\":{}}}",
        s.constants,
        ps.slots,
        ps.instruction_capacity_bytes,
        ps.root_capacity_bytes,
        os.value_capacity_bytes,
        ns.value_capacity_bytes
    );
    phase("end", Phase::Emit)?;
    Ok(())
}

fn optional(value: Option<usize>) -> String {
    value.map_or("null".into(), |n| n.to_string())
}
