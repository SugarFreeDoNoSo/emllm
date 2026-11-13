use crate::eval::{EvalError, EvalErrorKind, EvaluationStorage, magma_value};

use super::ExecutionPlan;

/// Ejecuta el plan en su orden original, con un buffer propio por llamada.
/// Los dos operandos se cargan antes de sobrescribir una posición reutilizada.
/// La salida conserva orden/duplicados y solo se entrega si todo el programa pasa.
///
/// El presupuesto cuenta nodos lógicos, incluido One, no posiciones del buffer.
/// Un rechazo previo tiene `node: None`; los fallos numéricos identifican el mismo
/// nodo y categoría que [`crate::eval::evaluate`], usando el mismo kernel exp/ln.
/// El plan y el programa permanecen inmutables después de éxito, error o reintento.
///
/// ```
/// use emllm::{execution::{plan, run, PlanLimits}, expr::Arena, program::compile};
/// let mut arena = Arena::new(3);
/// let one = arena.one().unwrap();
/// let e = arena.magma(&one, &one).unwrap();
/// let root = arena.magma(&e, &e).unwrap();
/// let program = compile(&arena, &[root], 3).unwrap();
/// drop(arena);
/// let plan = plan(&program, PlanLimits { max_nodes: 3, max_slots: 2 }).unwrap();
/// assert_eq!(plan.stats().slots, 2);
/// let result = run(&plan, 3).unwrap();
/// assert!((result[0] - (std::f64::consts::E.exp() - 1.0)).abs() <= 1e-12);
/// ```
pub fn run(plan: &ExecutionPlan<'_>, max_nodes: usize) -> Result<Vec<f64>, EvalError> {
    run_with_storage(plan, max_nodes).map(|(outputs, _)| outputs)
}

/// Mismo recorrido que [`run`], con capacidades reales de valores y salidas.
/// Los contadores son diagnósticos de almacenamiento, no una medida de RSS.
pub fn run_with_storage(
    plan: &ExecutionPlan<'_>,
    max_nodes: usize,
) -> Result<(Vec<f64>, EvaluationStorage), EvalError> {
    #[cfg(test)]
    crate::eval::probe::enter(crate::eval::probe::Entry::Planned);
    if plan.program.node_count() > max_nodes {
        return Err(EvalError {
            kind: EvalErrorKind::NodeLimit { limit: max_nodes },
            node: None,
        });
    }
    let mut values = vec![0.0; plan.slots];
    values[0] = 1.0;
    for (index, instruction) in plan.instructions.iter().enumerate() {
        let left = values[instruction.left];
        let right = values[instruction.right];
        values[instruction.destination] = magma_value(left, right, index + 1)?;
    }
    let outputs = plan.root_slots.iter().map(|&slot| values[slot]).collect();
    let storage = EvaluationStorage::capture(&values, &outputs);
    Ok((outputs, storage))
}
