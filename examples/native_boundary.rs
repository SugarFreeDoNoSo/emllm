//! Ejemplo acotado: cadena One/Magma estable, composición y lectura separadas.
//! No representa el Transformer ni mide generación de tokens.
use emllm::{
    eval::evaluate_with_storage,
    execution::{PlanLimits, plan, run_with_storage},
    model_io::F32Bits,
    native::{CompositionBuilder, NativeLimits, Phase, PreparedBase, compile_output},
};
use std::{
    error::Error,
    io::{self, Write},
    sync::Arc,
    time::Instant,
};

fn timed<T>(
    phase: Phase,
    work: impl FnOnce() -> Result<T, Box<dyn Error>>,
) -> Result<(T, f64), Box<dyn Error>> {
    event("start", phase)?;
    let start = Instant::now();
    let value = work()?;
    let seconds = start.elapsed().as_secs_f64();
    event("end", phase)?;
    Ok((value, seconds))
}

fn event(event: &str, phase: Phase) -> io::Result<()> {
    println!(
        "EML_NATIVE_PHASE {{\"schema_version\":1,\"event\":{event:?},\"phase\":{:?}}}",
        phase.as_str()
    );
    io::stdout().flush()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let steps: usize = args.next().map_or(Ok(65_536), |n| n.parse())?;
    if args.next().is_some() || steps == 0 {
        return Err("usage: native_boundary [positive_steps]".into());
    }
    let limits = NativeLimits::default();
    let zero_bits = F32Bits::new(0).unwrap();
    let (base, preparation_seconds) = timed(Phase::Base, || {
        Ok(Arc::new(PreparedBase::from_constants(
            &[zero_bits],
            limits,
        )?))
    })?;
    let (output, composition_seconds) = timed(Phase::Compose, || {
        let mut request = CompositionBuilder::new(base.clone(), limits)?;
        let zero = request.base_reference(&base.constant(zero_bits).unwrap())?;
        let mut root = request.one()?;
        // M(0,x) = 1-ln(x); con x=1 permanece 1. No se evalúa al construir.
        for _ in 0..steps {
            root = request.magma(&zero, &root)?;
        }
        Ok(request.finish(&[root])?)
    })?;
    let ((program, compilation), compile_seconds) =
        timed(Phase::Compile, || Ok(compile_output(&output, limits)?))?;
    let (plan, plan_seconds) = timed(Phase::Plan, || {
        Ok(plan(
            &program,
            PlanLimits {
                max_nodes: limits.reachable_nodes,
                max_slots: limits.numeric_slots,
            },
        )?)
    })?;
    let ((original, original_buffers), original_seconds) = timed(Phase::EvaluateOriginal, || {
        Ok(evaluate_with_storage(&program, limits.reachable_nodes)?)
    })?;
    let ((planned, planned_buffers), planned_seconds) = timed(Phase::EvaluatePlanned, || {
        Ok(run_with_storage(&plan, limits.reachable_nodes)?)
    })?;
    let (_, comparison_seconds) = timed(Phase::Compare, || {
        if original.len() != 1
            || planned.len() != 1
            || !original[0].is_finite()
            || !planned[0].is_finite()
            || (original[0] - 1.0).abs() > 1e-9
            || (planned[0] - original[0]).abs() > 1e-12 + 1e-12 * original[0].abs()
        {
            return Err("independent oracle or reader differential failed".into());
        }
        Ok(())
    })?;
    event("start", Phase::Emit)?;
    let b = base.storage();
    let o = output.storage();
    let p = program.storage();
    let ps = plan.stats();
    println!(
        "{{\"event\":\"boundary-verified\",\"output_kind\":\"eml-expressions\",\"steps\":{steps},\"roots\":{},\"value\":{},\"preparation_seconds\":{preparation_seconds},\"composition_seconds\":{composition_seconds},\"compile_seconds\":{compile_seconds},\"plan_seconds\":{plan_seconds},\"original_seconds\":{original_seconds},\"planned_seconds\":{planned_seconds},\"comparison_seconds\":{comparison_seconds},\"base_nodes\":{},\"new_nodes\":{},\"reachable_nodes\":{},\"depth\":{},\"base_vector_payload_bytes\":{},\"base_vector_capacity_bytes\":{},\"base_map_modeled_payload_bytes\":{},\"delta_pair_payload_bytes\":{},\"delta_pair_capacity_bytes\":{},\"output_root_capacity_bytes\":{},\"released_compose_map_modeled_payload_bytes\":{},\"compile_state_payload_bytes\":{},\"compile_state_capacity_bytes\":{},\"compile_remap_payload_bytes\":{},\"compile_remap_capacity_bytes\":{},\"compile_depth_payload_bytes\":{},\"compile_depth_capacity_bytes\":{},\"compile_stack_peak_payload_bytes\":{},\"compile_stack_capacity_bytes\":{},\"program_pair_capacity_bytes\":{},\"program_root_capacity_bytes\":{},\"plan_instruction_capacity_bytes\":{},\"plan_root_capacity_bytes\":{},\"numeric_slots\":{},\"original_value_capacity_bytes\":{},\"planned_value_capacity_bytes\":{}}}",
        o.roots,
        original[0],
        b.nodes,
        o.new_nodes,
        compilation.reachable_nodes,
        program.stats().depth,
        b.vector_payload_bytes,
        b.vector_capacity_bytes,
        b.pair_index_modeled_payload_bytes + b.constant_index_modeled_payload_bytes,
        o.pair_payload_bytes,
        o.pair_capacity_bytes,
        o.root_capacity_bytes,
        o.construction_index_modeled_payload_bytes,
        compilation.state_payload_bytes,
        compilation.state_capacity_bytes,
        compilation.remap_payload_bytes,
        compilation.remap_capacity_bytes,
        compilation.depth_payload_bytes,
        compilation.depth_capacity_bytes,
        compilation.stack_peak_payload_bytes,
        compilation.stack_capacity_bytes,
        p.pair_capacity_bytes,
        p.root_capacity_bytes,
        ps.instruction_capacity_bytes,
        ps.root_capacity_bytes,
        ps.slots,
        original_buffers.value_capacity_bytes,
        planned_buffers.value_capacity_bytes
    );
    event("end", Phase::Emit)?;
    Ok(())
}
