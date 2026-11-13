//! Mantisa fraccionaria y escala mediante el mismo constructor que el modelo.
use super::{Builder, add, exp, logarithm, neg, zero};
use crate::model_io::F32Bits;
use crate::synth::scalar::decode;

// H/D(x) = exp(A(±ln(2), ln(x))), x>0 en reales. El primer operando
// de A permanece fijo; no se escoge una orientación evaluando x.
fn scale_positive<B: Builder>(
    arena: &mut B,
    log_scale: &B::Ref,
    x: &B::Ref,
) -> Result<B::Ref, B::Error> {
    let log_x = logarithm(arena, x)?;
    let sum = add(arena, log_scale, &log_x)?;
    exp(arena, &sum)
}

pub(crate) fn synthesize<B: Builder>(arena: &mut B, value: F32Bits) -> Result<B::Ref, B::Error> {
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
