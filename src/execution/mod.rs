//! Planificación opt-in de temporales sobre un [`crate::program::Program`].
//! El orden y los índices lógicos se conservan; solo cambia el almacenamiento
//! numérico. Cada plan presta su programa inmutable y no contiene resultados.

mod plan;

pub use plan::{ExecutionPlan, PlanError, PlanLimits, PlanStats, plan};
