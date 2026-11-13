//! Expansiones candidatas del modelo, con identidades en `docs/synthesis.md`.
//! Solo construyen One/Magma; su dominio real no garantiza estabilidad F64.
//! Los argumentos de recíproco/raíz deben ser positivos en reales; los errores
//! numéricos se detectan al evaluar, sin clamps ni evaluación en construcción.

use crate::expr::{Arena, BuildError, NodeId};

use super::{Rational, logarithm, rational};

/// Candidatos iniciales A/E/F/N/S, conservando sus contratos y limitaciones.
pub use super::{add, exp, mul, neg, sub};

/// R(x)=exp(-ln(x))=1/x, para x>0 en reales.
pub fn reciprocal_positive(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let log = logarithm(arena, x)?;
    let negative = neg(arena, &log)?;
    exp(arena, &negative)
}

/// U(x)=exp((-1/2)*ln(x))=1/sqrt(x), para x>0 en reales.
pub fn inverse_sqrt(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let log = logarithm(arena, x)?;
    let half = rational(
        arena,
        Rational::new(-1, 2).expect("initial rational domain"),
    )?;
    let exponent = mul(arena, &half, &log)?;
    exp(arena, &exponent)
}

/// SiLU(x)=x/(1+exp(-x)). El denominador es positivo en reales.
pub fn silu(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let negative = neg(arena, x)?;
    let exponential = exp(arena, &negative)?;
    let one = arena.one()?;
    let denominator = add(arena, &one, &exponential)?;
    let inverse = reciprocal_positive(arena, &denominator)?;
    mul(arena, x, &inverse)
}
