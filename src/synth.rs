//! Expansiones algebraicas a nodos One/Magma.
//!
//! Las derivaciones están en `docs/synthesis.md`. Estas funciones solo componen
//! estructura: no evalúan los operandos ni introducen hojas numéricas. La validez
//! algebraica no garantiza estabilidad o ausencia de desbordamiento en `f64`.

use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, NodeId};

/// Racional normalizado de la frontera de construcción, no un nodo ejecutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    numerator: i32,
    denominator: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RationalError {
    ZeroDenominator,
    OutsideDomain { numerator: i32, denominator: u32 },
}

impl fmt::Display for RationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDenominator => write!(f, "rational denominator must be nonzero"),
            Self::OutsideDomain {
                numerator,
                denominator,
            } => write!(
                f,
                "normalized rational {numerator}/{denominator} is outside the initial domain"
            ),
        }
    }
}

impl Error for RationalError {}

impl Rational {
    /// Admite `abs(p)<=2` y `q<=2` después de normalizar, con q positivo.
    pub fn new(numerator: i32, denominator: u32) -> Result<Self, RationalError> {
        if denominator == 0 {
            return Err(RationalError::ZeroDenominator);
        }
        let mut a = numerator.unsigned_abs();
        let mut b = denominator;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        let numerator = (i64::from(numerator) / i64::from(a)) as i32;
        let denominator = denominator / a;
        if numerator.unsigned_abs() > 2 || denominator > 2 {
            return Err(RationalError::OutsideDomain {
                numerator,
                denominator,
            });
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub fn numerator(self) -> i32 {
        self.numerator
    }

    pub fn denominator(self) -> u32 {
        self.denominator
    }
}

/// E(x) = Magma(x, One).
pub fn exp(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let one = arena.one()?;
    arena.magma(x, &one)
}

/// z = Magma(One, exp(exp(One))); conserva la cancelación numérica.
pub fn zero(arena: &mut Arena) -> Result<NodeId, BuildError> {
    let one = arena.one()?;
    let e = exp(arena, &one)?;
    let ee = exp(arena, &e)?;
    arena.magma(&one, &ee)
}

/// L(y), usado internamente solo con argumentos demostrados positivos.
fn logarithm(arena: &mut Arena, y: &NodeId) -> Result<NodeId, BuildError> {
    let z = zero(arena)?;
    let reflected = arena.magma(&z, y)?;
    let exponential = exp(arena, &reflected)?;
    arena.magma(&z, &exponential)
}

/// P(x) = exp(x)-x >= 1 en aritmética real.
fn positive_residue(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let exponential = exp(arena, x)?;
    arena.magma(x, &exponential)
}

/// A(x,y) = exp(x) - (P(x)-y), sin restricción de signo en reales.
pub fn add(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let residue = positive_residue(arena, x)?;
    let log_residue = logarithm(arena, &residue)?;
    let ey = exp(arena, y)?;
    let difference = arena.magma(&log_residue, &ey)?;
    let exponential = exp(arena, &difference)?;
    arena.magma(x, &exponential)
}

/// N(x) = 1-(1+x).
pub fn neg(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    let one = arena.one()?;
    let incremented = add(arena, &one, x)?;
    let exponential = exp(arena, &incremented)?;
    let z = zero(arena)?;
    arena.magma(&z, &exponential)
}

pub fn sub(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    let negative = neg(arena, y)?;
    add(arena, x, &negative)
}

fn positive_product(arena: &mut Arena, a: &NodeId, b: &NodeId) -> Result<NodeId, BuildError> {
    let log_a = logarithm(arena, a)?;
    let log_b = logarithm(arena, b)?;
    let sum = add(arena, &log_a, &log_b)?;
    exp(arena, &sum)
}

/// Producto por descomposición en diferencias de términos positivos.
pub fn mul(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
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

/// Sintetiza el racional normalizado usando unidades, suma, logaritmo y negación.
pub fn rational(arena: &mut Arena, value: Rational) -> Result<NodeId, BuildError> {
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
