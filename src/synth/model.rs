//! Expansiones candidatas del modelo, con identidades en `docs/synthesis.md`.
//! Solo construyen One/Magma; su dominio real no garantiza estabilidad F64.
//! Los argumentos de recíproco/raíz deben ser positivos en reales; los errores
//! numéricos se detectan al evaluar, sin clamps ni evaluación en construcción.

pub use super::exp;
use super::structural::model as shared;
use crate::expr::{Arena, BuildError, NodeId};

/// N(x)=R_1(R_2(R_1(x)))=-x. Evita sumas de exponenciales anidadas.
pub fn neg(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    shared::neg(arena, x)
}

/// Suma desplazada M(L(128+x), E(128-y))=x+y, para x>-128 en reales.
/// Su rango numérico se verifica con el corpus; no es una suma universal F64.
pub fn add(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    shared::add(arena, x, y)
}

pub fn sub(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    shared::sub(arena, x, y)
}

/// Suma positiva por logaritmos: exp(ln(x)+ln(1+exp(ln(y)-ln(x)))).
/// Requiere x,y>0 y ln(x),ln(y)>-128 para las sumas desplazadas;
/// su estabilidad F64 depende también del rango de los logaritmos y su diferencia.
pub fn add_positive(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    shared::add_positive(arena, x, y)
}

/// Cociente positivo exp(ln(x)-ln(y)), con x,y>0 y ln(x)>-128 en reales.
/// Evita pasar un numerador grande por la expansión del producto firmado.
pub fn divide_positive(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    shared::divide_positive(arena, x, y)
}

/// Producto normalizado: u=x/64, v=y/64; (1+u)(1+v)-(1+u+v)=u*v.
/// Recupera x*y como 4096*(q+1/8)-512, donde q=u*v.
/// Dominio propuesto: |x|,|y|<=32 y x*y>-256; requiere verificación F64.
pub fn mul(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    shared::mul(arena, x, y)
}

/// R(x)=exp(-ln(x))=1/x, para x>0 en reales.
pub fn reciprocal_positive(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    shared::reciprocal_positive(arena, x)
}

/// U(x)=exp(-ln(x)/2)=1/sqrt(x), para x>0 y ln(x)>-128 en reales.
pub fn inverse_sqrt(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    shared::inverse_sqrt(arena, x)
}

/// SiLU(x)=x/(1+exp(-x)); M(-x,E(-1)) construye el denominador directamente.
pub fn silu(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    shared::silu(arena, x)
}
