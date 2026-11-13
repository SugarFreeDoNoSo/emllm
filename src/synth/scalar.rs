//! Síntesis candidata de metadatos F32 mediante mantisa fraccionaria One/Magma.
//! Las identidades y dominios están en `docs/synthesis.md`; construir no certifica
//! éxito numérico. La aceptación requiere evaluar el corpus con sus tolerancias.

use crate::expr::{Arena, BuildError, NodeId};
pub use crate::model_io::F32Bits;

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

/// Expande bits finitos a un DAG cerrado. Solo las operaciones enteras de decode
/// y los bits de mantisa deciden la construcción; no usa el valor F32/F64 ni un
/// evaluador. Los errores de presupuesto son los de la arena y no entregan raíz.
/// No hay caché de handles entre arenas. `Rational` conserva su dominio original.
///
/// El algoritmo es candidato, no una promesa para todos los F32: por ejemplo,
/// negativizar magnitudes muy grandes con N puede desbordar durante evaluación.
pub fn synthesize(arena: &mut Arena, value: F32Bits) -> Result<NodeId, BuildError> {
    super::structural::scalar::synthesize(arena, value)
}
