//! Primer bloque real con variantes diagnóstica y residual preparadas sin IDs.
use emllm::{
    eval::EvalError,
    execution::{PlanLimits, plan, run},
    model_io::{LoadLimits, read_stories260k},
    native::{
        NativeError, NativeErrorKind, NativeLimits, NativeOutput, NativePrefix, PreparedBase,
        compile_output, decode_with_events, model::PreparedBlock,
    },
    program::Program,
};
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

fn phase(event: &str, name: &str) -> io::Result<()> {
    println!("EML_NATIVE_PHASE {{\"schema_version\":1,\"event\":{event:?},\"phase\":{name:?}}}");
    io::stdout().flush()
}

/// Propaga dependencia estructural, no orden numérico de raíces: una raíz
/// posterior al nodo fallido puede pertenecer a una rama independiente.
fn first_affected_root(program: &Program, failed: usize) -> Result<usize, Box<dyn Error>> {
    if failed >= program.node_count() {
        return Err("failure node outside compiled program".into());
    }
    let mut affected = Vec::new();
    affected.try_reserve_exact(program.node_count())?;
    affected.resize(program.node_count(), false);
    affected[failed] = true;
    for (offset, &[left, right]) in program.pairs().iter().enumerate().skip(failed) {
        affected[offset + 1] = affected[left] || affected[right];
    }
    program
        .roots()
        .iter()
        .position(|&root| affected[root])
        .ok_or_else(|| "failure does not reach any diagnostic root".into())
}

/// Replay exclusivamente tras fallo. Mantiene abierta la fase fallida del
/// supervisor; su tiempo/RSS incluyen esta investigación y se etiquetan aparte.
/// Los tiempos de la corrida inicial siguen en su propio directorio de evidencia.
fn diagnose_failure(
    output: &NativeOutput,
    block: &PreparedBlock,
    limits: NativeLimits,
    error: &NativeError,
    directory: &Path,
    variant: &str,
) -> Result<(), Box<dyn Error>> {
    let NativeErrorKind::Evaluation(EvalError {
        node: Some(node), ..
    }) = &error.kind
    else {
        return Ok(());
    };
    println!(
        "{{\"event\":\"failure-diagnostic-start\",\"variant\":{variant:?},\"compiled_node\":{node}}}"
    );
    io::stdout().flush()?;
    let start = Instant::now();
    let (program, _) = compile_output(output, limits)?;
    let root = first_affected_root(&program, *node)?;
    let stage = block
        .stages()
        .iter()
        .find(|stage| stage.roots.contains(&root))
        .ok_or("failure root outside stage map")?;
    let coordinates = stage.coordinates(root).ok_or("missing coordinates")?;
    let (position, key) = stage.position_and_key(root).ok_or("missing position")?;
    let plan = plan(
        &program,
        PlanLimits {
            max_nodes: limits.reachable_nodes,
            max_slots: limits.numeric_slots,
        },
    )?;
    let planned_error = run(&plan, limits.reachable_nodes).err();
    let same_error = match &error.kind {
        NativeErrorKind::Evaluation(original) => planned_error.as_ref() == Some(original),
        _ => false,
    };
    let key = key.map_or_else(|| "null".to_owned(), |key| key.to_string());
    let record = format!(
        "{{\"event\":\"failure-diagnostic\",\"variant\":{variant:?},\"phase\":{:?},\"error\":{:?},\"compiled_node\":{node},\"first_affected_root\":{root},\"closed_root_node\":{},\"stage\":{:?},\"coordinates\":{coordinates:?},\"position\":{position},\"key_position\":{key},\"planned_error\":{:?},\"same_error\":{same_error},\"reachable_nodes\":{},\"slots\":{},\"replay_seconds\":{}}}",
        error.phase.as_str(),
        error.to_string(),
        output.root_indices()[root],
        stage.name,
        format!("{planned_error:?}"),
        program.node_count(),
        plan.stats().slots,
        start.elapsed().as_secs_f64(),
    );
    fs::write(
        directory.join(format!("{variant}.failure.json")),
        format!("{record}\n"),
    )?;
    println!("{record}");
    io::stdout().flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let package = args
        .next()
        .ok_or("usage: verify_native_block PACKAGE OUTPUT IDS [limit=value ...]")?;
    let directory = PathBuf::from(args.next().ok_or("missing output")?);
    let ids = args
        .next()
        .ok_or("missing ids")?
        .split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut limits = NativeLimits::default();
    for argument in args {
        let (name, value) = argument.split_once('=').ok_or("expected limit=value")?;
        let value = value.parse()?;
        match name {
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
    fs::create_dir(&directory)?;
    phase("start", "load")?;
    let mut bytes = Vec::new();
    File::open(package)?
        .take(limits.package_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    let imported = read_stories260k(
        &bytes,
        LoadLimits {
            max_bytes: limits.package_bytes,
            max_tensors: limits.tensors,
        },
    )?;
    phase("end", "load")?;
    phase("start", "base")?;
    let start = Instant::now();
    let base = Arc::new(PreparedBase::from_stories260k(&imported, limits)?);
    let base_seconds = start.elapsed().as_secs_f64();
    drop(imported);
    drop(bytes);
    phase("end", "base")?;
    println!(
        "{{\"event\":\"base\",\"nodes\":{},\"seconds\":{base_seconds},\"ids\":{ids:?}}}",
        base.program().node_count()
    );
    for diagnostic in [true, false] {
        let variant = if diagnostic { "diagnostic" } else { "residual" };
        println!("{{\"event\":\"variant\",\"name\":{variant:?}}}");
        io::stdout().flush()?;
        phase("start", "template")?;
        let start = Instant::now();
        let block = PreparedBlock::prepare(base.clone(), ids.len(), diagnostic, limits)?;
        let template_seconds = start.elapsed().as_secs_f64();
        phase("end", "template")?;
        let s = block.model().template().stats();
        println!(
            "{{\"event\":\"template\",\"variant\":{variant:?},\"seconds\":{template_seconds},\"instructions\":{},\"built_instructions\":{},\"ports\":{},\"roots\":{},\"pair_capacity_bytes\":{},\"root_capacity_bytes\":{},\"released_index_modeled_payload_bytes\":{}}}",
            s.instructions,
            s.built_instructions,
            s.ports,
            s.roots,
            s.pair_capacity_bytes,
            s.root_capacity_bytes,
            s.construction_index_modeled_payload_bytes
        );
        io::stdout().flush()?;
        let mut metadata = BufWriter::new(File::create(
            directory.join(format!("{variant}.stages.jsonl")),
        )?);
        for stage in block.stages() {
            writeln!(
                metadata,
                "{{\"name\":{:?},\"shape\":{:?},\"start\":{},\"count\":{},\"layout\":{:?}}}",
                stage.name,
                stage.shape.dimensions(),
                stage.roots.start,
                stage.roots.len(),
                format!("{:?}", stage.layout)
            )?;
        }
        metadata.flush()?;
        phase("start", "encode")?;
        let prefix = NativePrefix::encode(
            base.clone(),
            &ids,
            &(0..ids.len()).collect::<Vec<_>>(),
            ids.len(),
            limits,
        )?;
        phase("end", "encode")?;
        phase("start", "compose")?;
        let start = Instant::now();
        let (output, cs) = block.model().compose_with_stats(&prefix, limits)?;
        let compose_seconds = start.elapsed().as_secs_f64();
        phase("end", "compose")?;
        let s = output.storage();
        println!(
            "{{\"event\":\"composition\",\"variant\":{variant:?},\"seconds\":{compose_seconds},\"closed_nodes\":{},\"new_nodes\":{},\"pair_capacity_bytes\":{},\"root_capacity_bytes\":{},\"released_index_modeled_payload_bytes\":{},\"remap_capacity_bytes\":{},\"port_capacity_bytes\":{}}}",
            s.closed_nodes,
            s.new_nodes,
            s.pair_capacity_bytes,
            s.root_capacity_bytes,
            s.construction_index_modeled_payload_bytes,
            cs.remap_capacity_bytes,
            cs.port_capacity_bytes
        );
        io::stdout().flush()?;
        let decoded = match decode_with_events(&output, limits, |p, start| {
            phase(if start { "start" } else { "end" }, p.as_str()).expect("phase output")
        }) {
            Ok(decoded) => decoded,
            Err(error) => {
                println!(
                    "{{\"event\":\"failed\",\"variant\":{variant:?},\"phase\":{:?},\"error\":{:?}}}",
                    error.phase.as_str(),
                    error.to_string()
                );
                io::stdout().flush()?;
                if let Err(diagnostic) =
                    diagnose_failure(&output, &block, limits, &error, &directory, variant)
                {
                    eprintln!("failure diagnosis incomplete: {diagnostic}");
                }
                return Err(error.into());
            }
        };
        phase("start", "emit")?;
        for (name, values) in [
            ("original", &decoded.original),
            ("planned", &decoded.planned),
        ] {
            let mut file = BufWriter::new(File::create(
                directory.join(format!("{variant}.{name}.f64")),
            )?);
            for value in values {
                file.write_all(&value.to_le_bytes())?;
            }
            file.flush()?;
        }
        let mut roots = BufWriter::new(File::create(
            directory.join(format!("{variant}.roots.u64")),
        )?);
        for root in output.root_indices() {
            roots.write_all(&(*root as u64).to_le_bytes())?;
        }
        roots.flush()?;
        println!(
            "{{\"event\":\"decoded\",\"variant\":{variant:?},\"reachable_nodes\":{},\"slots\":{},\"compile_seconds\":{},\"plan_seconds\":{},\"original_seconds\":{},\"planned_seconds\":{}}}",
            decoded.stats.compilation.reachable_nodes,
            decoded.stats.plan.slots,
            decoded.stats.compile_seconds,
            decoded.stats.plan_seconds,
            decoded.stats.original_seconds,
            decoded.stats.planned_seconds
        );
        phase("end", "emit")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use emllm::{expr::Arena, program::compile, synth};

    #[test]
    fn failed_dependency_maps_to_first_root_even_with_repeated_and_independent_roots() {
        let mut arena = Arena::new(100);
        let one = arena.one().unwrap();
        let zero = synth::zero(&mut arena).unwrap();
        let failure = arena.magma(&zero, &zero).unwrap();
        let parent = arena.magma(&one, &failure).unwrap();
        let independent = arena.magma(&one, &one).unwrap();
        let program = compile(
            &arena,
            &[one.clone(), zero, parent.clone(), independent, parent, one],
            100,
        )
        .unwrap();
        let error = emllm::eval::evaluate(&program, 100).unwrap_err();
        assert_eq!(
            first_affected_root(&program, error.node.unwrap()).unwrap(),
            2
        );
        assert!(first_affected_root(&program, program.node_count()).is_err());
    }
}
