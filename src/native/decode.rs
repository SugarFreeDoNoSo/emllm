//! Lectura numérica explícita, separada del compositor y sus constructores.
use super::{NativeError, NativeErrorKind, NativeLimits, NativeOutput, Phase, Resource};
use crate::{
    eval::{EvaluationStorage, evaluate_with_storage},
    execution::{PlanLimits, PlanStats, plan, run_with_storage},
    program::{DenseCompileError, DenseCompileStorage, Program, ProgramStorage, compile_dense},
};
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct DecodeStats {
    pub compile_seconds: f64,
    pub plan_seconds: f64,
    pub original_seconds: f64,
    pub planned_seconds: f64,
    pub comparison_seconds: f64,
    pub compilation: DenseCompileStorage,
    pub program: ProgramStorage,
    pub plan: PlanStats,
    pub original_buffers: EvaluationStorage,
    pub planned_buffers: EvaluationStorage,
}

#[derive(Debug)]
pub struct DecodedOutput {
    pub original: Vec<f64>,
    pub planned: Vec<f64>,
    /// Capacidades observadas durante lectura; programa/plan se liberan al retornar.
    pub stats: DecodeStats,
}

/// Clausura validada de una salida ya cerrada. Los presupuestos distinguen
/// fuente residente, raíces y nodos alcanzables; no admite pares arbitrarios.
pub fn compile_output(
    output: &NativeOutput,
    limits: NativeLimits,
) -> Result<(Program, DenseCompileStorage), NativeError> {
    limits.check(Phase::Compile, Resource::ClosedNodes, output.node_count())?;
    limits.check(Phase::Compile, Resource::Roots, output.root_indices().len())?;
    compile_dense(
        |i| output.node(i),
        output.node_count(),
        output.root_indices(),
        limits.reachable_nodes,
    )
    .map_err(|error| NativeError {
        phase: Phase::Compile,
        kind: match error {
            DenseCompileError::Graph(error) => NativeErrorKind::Compile(error),
            DenseCompileError::Allocation { storage } => NativeErrorKind::Allocation {
                resource: Resource::ReachableNodes,
                detail: storage.into(),
            },
            DenseCompileError::SizeOverflow { storage: _storage } => {
                NativeErrorKind::SizeOverflow {
                    resource: Resource::ReachableNodes,
                }
            }
        },
    })
}

/// Compila, planifica y ejecuta ambos lectores, comprobando todas las raíces con
/// `1e-12 + 1e-12*abs(original)`. No selecciona token ni consulta referencias externas.
pub fn decode(output: &NativeOutput, limits: NativeLimits) -> Result<DecodedOutput, NativeError> {
    decode_with_events(output, limits, |_, _| {})
}

/// Observa inicio (`true`)/fin (`false`) de cada fase fuera de su cronómetro.
/// Ante error la fase queda abierta para que el supervisor conserve el fallo.
pub fn decode_with_events(
    output: &NativeOutput,
    limits: NativeLimits,
    mut event: impl FnMut(Phase, bool),
) -> Result<DecodedOutput, NativeError> {
    event(Phase::Compile, true);
    let start = Instant::now();
    let (program, compilation) = compile_output(output, limits)?;
    let compile_seconds = start.elapsed().as_secs_f64();
    event(Phase::Compile, false);
    event(Phase::Plan, true);
    let start = Instant::now();
    let plan = plan(
        &program,
        PlanLimits {
            max_nodes: limits.reachable_nodes,
            max_slots: limits.numeric_slots,
        },
    )
    .map_err(|e| failure(Phase::Plan, NativeErrorKind::Plan(e)))?;
    let plan_seconds = start.elapsed().as_secs_f64();
    event(Phase::Plan, false);
    event(Phase::EvaluateOriginal, true);
    let start = Instant::now();
    let (original, original_buffers) = evaluate_with_storage(&program, limits.reachable_nodes)
        .map_err(|e| failure(Phase::EvaluateOriginal, NativeErrorKind::Evaluation(e)))?;
    let original_seconds = start.elapsed().as_secs_f64();
    event(Phase::EvaluateOriginal, false);
    event(Phase::EvaluatePlanned, true);
    let start = Instant::now();
    let (planned, planned_buffers) = run_with_storage(&plan, limits.reachable_nodes)
        .map_err(|e| failure(Phase::EvaluatePlanned, NativeErrorKind::Evaluation(e)))?;
    let planned_seconds = start.elapsed().as_secs_f64();
    event(Phase::EvaluatePlanned, false);
    event(Phase::Compare, true);
    let start = Instant::now();
    for (root, (&a, &b)) in original.iter().zip(&planned).enumerate() {
        if !a.is_finite() || !b.is_finite() || (a - b).abs() > 1e-12 + 1e-12 * a.abs() {
            return Err(failure(
                Phase::Compare,
                NativeErrorKind::Differential { root },
            ));
        }
    }
    let comparison_seconds = start.elapsed().as_secs_f64();
    event(Phase::Compare, false);
    Ok(DecodedOutput {
        original,
        planned,
        stats: DecodeStats {
            compile_seconds,
            plan_seconds,
            original_seconds,
            planned_seconds,
            comparison_seconds,
            compilation,
            program: program.storage(),
            plan: plan.stats(),
            original_buffers,
            planned_buffers,
        },
    })
}
fn failure(phase: Phase, kind: NativeErrorKind) -> NativeError {
    NativeError { phase, kind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::EvalErrorKind,
        model_io::F32Bits,
        native::{CompositionBuilder, PreparedBase},
    };
    use std::sync::Arc;

    #[test]
    fn repeated_roots_match_and_a_domain_failure_only_appears_during_reading() {
        let limits = NativeLimits::default();
        let base =
            Arc::new(PreparedBase::from_constants(&[F32Bits::new(0).unwrap()], limits).unwrap());
        let mut request = CompositionBuilder::new(base.clone(), limits).unwrap();
        let zero = request
            .base_reference(&base.constant(F32Bits::new(0).unwrap()).unwrap())
            .unwrap();
        let one = request.one().unwrap();
        request.magma(&zero, &zero).unwrap(); // No alcanzable: no debe evaluarse.
        let output = request.finish(&[one.clone(), zero, one]).unwrap();
        let values = decode(&output, limits).unwrap();
        assert_eq!(values.original, vec![1.0, 0.0, 1.0]);
        assert_eq!(values.original, values.planned);
        assert!(values.stats.compilation.reachable_nodes < output.node_count());
        let mut request = CompositionBuilder::new(base.clone(), limits).unwrap();
        let zero = request
            .base_reference(&base.constant(F32Bits::new(0).unwrap()).unwrap())
            .unwrap();
        let invalid = request.magma(&zero, &zero).unwrap();
        let output = request.finish(&[invalid]).unwrap();
        let (program, _) = compile_output(&output, limits).unwrap();
        let plan = plan(
            &program,
            PlanLimits {
                max_nodes: limits.reachable_nodes,
                max_slots: limits.numeric_slots,
            },
        )
        .unwrap();
        assert_eq!(
            evaluate_with_storage(&program, limits.reachable_nodes).unwrap_err(),
            run_with_storage(&plan, limits.reachable_nodes).unwrap_err()
        );
        let mut events = Vec::new();
        let failure =
            decode_with_events(&output, limits, |phase, start| events.push((phase, start)))
                .unwrap_err();
        assert_eq!(failure.phase, Phase::EvaluateOriginal);
        assert_eq!(
            events,
            [
                (Phase::Compile, true),
                (Phase::Compile, false),
                (Phase::Plan, true),
                (Phase::Plan, false),
                (Phase::EvaluateOriginal, true)
            ]
        );
        assert!(
            matches!(failure.kind, NativeErrorKind::Evaluation(error) if error.kind == EvalErrorKind::LogDomain && error.node.is_some())
        );
    }
}
