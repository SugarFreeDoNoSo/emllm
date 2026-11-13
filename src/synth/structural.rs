//! Implementación única de las expansiones; el constructor decide almacenamiento,
//! procedencia, internado y errores. No hay frontera numérica en este módulo.

use super::Rational;
use crate::{
    expr::{Arena, BuildError, NodeId},
    model_io::F32Bits,
};

pub(crate) mod model;
pub(crate) mod scalar;

pub(crate) trait Builder: Sized {
    type Ref: Clone;
    type Error;

    fn one(&mut self) -> Result<Self::Ref, Self::Error>;
    fn magma(&mut self, left: &Self::Ref, right: &Self::Ref) -> Result<Self::Ref, Self::Error>;

    /// La arena expande bits; una plantilla podrá resolver una constante de base.
    fn constant(&mut self, bits: F32Bits) -> Result<Self::Ref, Self::Error> {
        scalar::synthesize(self, bits)
    }
}

impl Builder for Arena {
    type Ref = NodeId;
    type Error = BuildError;

    fn one(&mut self) -> Result<NodeId, BuildError> {
        Arena::one(self)
    }
    fn magma(&mut self, left: &NodeId, right: &NodeId) -> Result<NodeId, BuildError> {
        Arena::magma(self, left, right)
    }
}

pub(crate) fn exp<B: Builder>(arena: &mut B, x: &B::Ref) -> Result<B::Ref, B::Error> {
    let one = arena.one()?;
    arena.magma(x, &one)
}

pub(crate) fn zero<B: Builder>(arena: &mut B) -> Result<B::Ref, B::Error> {
    let one = arena.one()?;
    let e = exp(arena, &one)?;
    let ee = exp(arena, &e)?;
    arena.magma(&one, &ee)
}

pub(crate) fn logarithm<B: Builder>(arena: &mut B, y: &B::Ref) -> Result<B::Ref, B::Error> {
    let z = zero(arena)?;
    let reflected = arena.magma(&z, y)?;
    let exponential = exp(arena, &reflected)?;
    arena.magma(&z, &exponential)
}

fn positive_residue<B: Builder>(arena: &mut B, x: &B::Ref) -> Result<B::Ref, B::Error> {
    let exponential = exp(arena, x)?;
    arena.magma(x, &exponential)
}

pub(crate) fn add<B: Builder>(arena: &mut B, x: &B::Ref, y: &B::Ref) -> Result<B::Ref, B::Error> {
    let residue = positive_residue(arena, x)?;
    let log_residue = logarithm(arena, &residue)?;
    let ey = exp(arena, y)?;
    let difference = arena.magma(&log_residue, &ey)?;
    let exponential = exp(arena, &difference)?;
    arena.magma(x, &exponential)
}

pub(crate) fn neg<B: Builder>(arena: &mut B, x: &B::Ref) -> Result<B::Ref, B::Error> {
    let one = arena.one()?;
    let incremented = add(arena, &one, x)?;
    let exponential = exp(arena, &incremented)?;
    let z = zero(arena)?;
    arena.magma(&z, &exponential)
}

pub(crate) fn sub<B: Builder>(arena: &mut B, x: &B::Ref, y: &B::Ref) -> Result<B::Ref, B::Error> {
    let negative = neg(arena, y)?;
    add(arena, x, &negative)
}

fn positive_product<B: Builder>(arena: &mut B, a: &B::Ref, b: &B::Ref) -> Result<B::Ref, B::Error> {
    let log_a = logarithm(arena, a)?;
    let log_b = logarithm(arena, b)?;
    let sum = add(arena, &log_a, &log_b)?;
    exp(arena, &sum)
}

pub(crate) fn mul<B: Builder>(arena: &mut B, x: &B::Ref, y: &B::Ref) -> Result<B::Ref, B::Error> {
    let ex = exp(arena, x)?;
    let ey = exp(arena, y)?;
    let px = positive_residue(arena, x)?;
    let py = positive_residue(arena, y)?;
    let u = positive_product(arena, &ex, &ey)?;
    let v = positive_product(arena, &ex, &py)?;
    let w = positive_product(arena, &px, &ey)?;
    let t = positive_product(arena, &px, &py)?;
    let first = sub(arena, &u, &v)?;
    let second = sub(arena, &w, &t)?;
    sub(arena, &first, &second)
}

pub(crate) fn rational<B: Builder>(arena: &mut B, value: Rational) -> Result<B::Ref, B::Error> {
    if value.numerator == 0 {
        return zero(arena);
    }
    let one = arena.one()?;
    let magnitude = if value.denominator == 2 {
        let two = add(arena, &one, &one)?;
        let log_two = logarithm(arena, &two)?;
        let negative = neg(arena, &log_two)?;
        exp(arena, &negative)?
    } else if value.numerator.unsigned_abs() == 2 {
        add(arena, &one, &one)?
    } else {
        one
    };
    if value.numerator < 0 {
        neg(arena, &magnitude)
    } else {
        Ok(magnitude)
    }
}
