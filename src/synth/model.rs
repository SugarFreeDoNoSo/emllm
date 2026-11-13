//! Expansiones candidatas del modelo, con identidades en `docs/synthesis.md`.
//! Solo construyen One/Magma; su dominio real no garantiza estabilidad F64.
//! Los argumentos de recíproco/raíz deben ser positivos en reales; los errores
//! numéricos se detectan al evaluar, sin clamps ni evaluación en construcción.

use crate::expr::{Arena, BuildError, NodeId};

use super::{
    Rational, logarithm, rational,
    scalar::{F32Bits, synthesize},
    zero,
};

pub use super::exp;

fn log_constant(arena: &mut Arena, bits: u32) -> Result<NodeId, BuildError> {
    let value = synthesize(arena, F32Bits::new(bits).expect("finite fixed metadata"))?;
    logarithm(arena, &value)
}

// R_c(x)=M(ln(c),E(x))=c-x. Solo se evalúa al ejecutar el DAG.
fn reflect(arena: &mut Arena, log_center: &NodeId, x: &NodeId) -> Result<NodeId, BuildError> {
    let exponential = exp(arena, x)?;
    arena.magma(log_center, &exponential)
}

/// N(x)=R_1(R_2(R_1(x)))=-x. Evita sumas de exponenciales anidadas.
pub fn neg(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let z = zero(arena)?;
    let log_two = log_constant(arena, 0x4000_0000)?;
    let first = reflect(arena, &z, x)?;
    let second = reflect(arena, &log_two, &first)?;
    reflect(arena, &z, &second)
}

// T_c(x)=R_c(N(x))=c+x. El centro es metadata fija de cada expansión.
fn translate(arena: &mut Arena, x: &NodeId, center: u32) -> Result<NodeId, BuildError> {
    let log_center = log_constant(arena, center)?;
    let negative = neg(arena, x)?;
    reflect(arena, &log_center, &negative)
}

/// Suma desplazada M(L(128+x), E(128-y))=x+y, para x>-128 en reales.
/// Su rango numérico se verifica con el corpus; no es una suma universal F64.
pub fn add(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let shifted = translate(arena, x, 0x4300_0000)?;
    let log_shifted = logarithm(arena, &shifted)?;
    let log_center = log_constant(arena, 0x4300_0000)?;
    let reflected = reflect(arena, &log_center, y)?;
    let exponential = exp(arena, &reflected)?;
    arena.magma(&log_shifted, &exponential)
}

pub fn sub(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let negative = neg(arena, y)?;
    add(arena, x, &negative)
}

// M(x,E(-1))=exp(x)+1, sin exponenciar el valor exp(x) una segunda vez.
fn one_plus_exp(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let negative_one = rational(
        arena,
        Rational::new(-1, 1).expect("initial rational domain"),
    )?;
    let inverse_e = exp(arena, &negative_one)?;
    arena.magma(x, &inverse_e)
}

/// Suma positiva por logaritmos: exp(ln(x)+ln(1+exp(ln(y)-ln(x)))).
/// Requiere x,y>0 y ln(x),ln(y)>-128 para las sumas desplazadas;
/// su estabilidad F64 depende también del rango de los logaritmos y su diferencia.
pub fn add_positive(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let log_x = logarithm(arena, x)?;
    let log_y = logarithm(arena, y)?;
    let difference = sub(arena, &log_y, &log_x)?;
    let increment = one_plus_exp(arena, &difference)?;
    let log_increment = logarithm(arena, &increment)?;
    let log_sum = add(arena, &log_x, &log_increment)?;
    exp(arena, &log_sum)
}

/// Cociente positivo exp(ln(x)-ln(y)), con x,y>0 y ln(x)>-128 en reales.
/// Evita pasar un numerador grande por la expansión del producto firmado.
pub fn divide_positive(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let log_x = logarithm(arena, x)?;
    let log_y = logarithm(arena, y)?;
    let difference = sub(arena, &log_x, &log_y)?;
    exp(arena, &difference)
}

// s*x = M(A(ln(s), L(128+x)), E(128*s)), x>-128, s>0.
// Los dos parámetros son constantes fijadas conjuntamente, no datos del oráculo.
fn scale_bounded(
    arena: &mut Arena,
    x: &NodeId,
    scale: u32,
    offset: u32,
) -> Result<NodeId, BuildError> {
    let shifted = translate(arena, x, 0x4300_0000)?;
    let log_shifted = logarithm(arena, &shifted)?;
    let log_scale = log_constant(arena, scale)?;
    let log_product = super::add(arena, &log_scale, &log_shifted)?;
    let offset = synthesize(arena, F32Bits::new(offset).expect("finite fixed metadata"))?;
    let exponential = exp(arena, &offset)?;
    arena.magma(&log_product, &exponential)
}

/// Producto normalizado: u=x/64, v=y/64; (1+u)(1+v)-(1+u+v)=u*v.
/// Recupera x*y como 4096*(q+1/8)-512, donde q=u*v.
/// Dominio propuesto: |x|,|y|<=32 y x*y>-256; requiere verificación F64.
pub fn mul(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let u = scale_bounded(arena, x, 0x3c80_0000, 0x4000_0000)?;
    let v = scale_bounded(arena, y, 0x3c80_0000, 0x4000_0000)?;
    let one = arena.one()?;
    let a = super::add(arena, &one, &u)?;
    let b = super::add(arena, &one, &v)?;
    let log_a = logarithm(arena, &a)?;
    let log_b = logarithm(arena, &b)?;
    let log_product = super::add(arena, &log_a, &log_b)?;
    let offset = super::add(arena, &a, &v)?;
    let exponential = exp(arena, &offset)?;
    let normalized = arena.magma(&log_product, &exponential)?;
    let shifted = translate(arena, &normalized, 0x3e00_0000)?; // q+1/8
    let log_shifted = logarithm(arena, &shifted)?;
    let log_scale = log_constant(arena, 0x4580_0000)?; // ln(4096)
    let scaled_log = add(arena, &log_scale, &log_shifted)?;
    let offset = synthesize(arena, F32Bits::new(0x4400_0000).expect("fixed 512"))?;
    let exponential = exp(arena, &offset)?;
    arena.magma(&scaled_log, &exponential)
}

/// R(x)=exp(-ln(x))=1/x, para x>0 en reales.
pub fn reciprocal_positive(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let log = logarithm(arena, x)?;
    let negative = neg(arena, &log)?;
    exp(arena, &negative)
}

/// U(x)=exp(-ln(x)/2)=1/sqrt(x), para x>0 y ln(x)>-128 en reales.
pub fn inverse_sqrt(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let log = logarithm(arena, x)?;
    let half_log = scale_bounded(arena, &log, 0x3f00_0000, 0x4280_0000)?;
    let exponent = neg(arena, &half_log)?;
    exp(arena, &exponent)
}

/// SiLU(x)=x/(1+exp(-x)); M(-x,E(-1)) construye el denominador directamente.
pub fn silu(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let negative = neg(arena, x)?;
    let denominator = one_plus_exp(arena, &negative)?;
    let inverse = reciprocal_positive(arena, &denominator)?;
    mul(arena, x, &inverse)
}
