//! Síntesis candidata de metadatos F32 mediante mantisa fraccionaria One/Magma.
//! Las identidades y dominios están en `docs/synthesis.md`; construir no certifica
//! éxito numérico. La aceptación requiere evaluar el corpus con sus tolerancias.

use crate::expr::{Arena, BuildError, NodeId};
pub use crate::model_io::F32Bits;

use super::{add, exp, logarithm, neg, zero};

/// Racional diádico exacto: `(-1)^negative * mantissa * 2^exponent`.
/// Metadatos enteros, no una instrucción. La mantisa no nula está normalizada a
/// un entero impar; ambos ceros tienen mantisa/exponente cero y conservan signo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dyadic {
    pub negative: bool,
    pub mantissa: u32,
    pub exponent: i32,
}

pub fn decode(value: F32Bits) -> Dyadic {
    let bits = value.bits();
    let biased = (bits >> 23) & 0xff;
    let fraction = bits & 0x7f_ffff;
    let (mut mantissa, mut exponent) = if biased == 0 {
        (fraction, -149)
    } else {
        (fraction | 0x80_0000, biased as i32 - 150)
    };
    if mantissa == 0 {
        exponent = 0;
    } else {
        let trailing = mantissa.trailing_zeros();
        mantissa >>= trailing;
        exponent += trailing as i32;
    }
    Dyadic {
        negative: bits >> 31 != 0,
        mantissa,
        exponent,
    }
}

// H/D(x) = exp(A(±ln(2), ln(x))), x>0 en reales. El primer operando
// de A permanece fijo; no se escoge una orientación evaluando x.
fn scale_positive(arena: &mut Arena, log_scale: &NodeId, x: &NodeId) -> Result<NodeId, BuildError> {
    let log_x = logarithm(arena, x)?;
    let sum = add(arena, log_scale, &log_x)?;
    exp(arena, &sum)
}

/// Expande bits finitos a un DAG cerrado. Solo las operaciones enteras de decode
/// y los bits de mantisa deciden la construcción; no usa el valor F32/F64 ni un
/// evaluador. Los errores de presupuesto son los de la arena y no entregan raíz.
/// No hay caché de handles entre arenas. `Rational` conserva su dominio original.
///
/// El algoritmo es candidato, no una promesa para todos los F32: por ejemplo,
/// negativizar magnitudes muy grandes con N puede desbordar durante evaluación.
pub fn synthesize(arena: &mut Arena, value: F32Bits) -> Result<NodeId, BuildError> {
    let dyadic = decode(value);
    if dyadic.mantissa == 0 {
        return zero(arena);
    }
    let one = arena.one()?;
    let two = add(arena, &one, &one)?;
    let log_two = logarithm(arena, &two)?;
    let negative_log_two = neg(arena, &log_two)?;
    let mut magnitude = exp(arena, &negative_log_two)?;
    let width = 32 - dyadic.mantissa.leading_zeros();
    // La mantisa normalizada es impar: su primer bit 1 ya está en 1/2.
    for bit in 1..width {
        if dyadic.mantissa & (1 << bit) != 0 {
            magnitude = add(arena, &one, &magnitude)?;
        }
        magnitude = scale_positive(arena, &negative_log_two, &magnitude)?;
    }
    let shift = dyadic.exponent + width as i32;
    let log_scale = if shift < 0 {
        &negative_log_two
    } else {
        &log_two
    };
    for _ in 0..shift.unsigned_abs() {
        magnitude = scale_positive(arena, log_scale, &magnitude)?;
    }
    if dyadic.negative {
        neg(arena, &magnitude)
    } else {
        Ok(magnitude)
    }
}
