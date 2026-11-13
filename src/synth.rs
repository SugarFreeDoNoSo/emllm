//! Expansiones algebraicas a nodos One/Magma.
//!
//! Las derivaciones están en `docs/synthesis.md`. Estas funciones solo componen
//! estructura: no evalúan los operandos ni introducen hojas numéricas. La validez
//! algebraica no garantiza estabilidad o ausencia de desbordamiento en `f64`.

use std::error::Error;
use std::fmt;

use crate::expr::{Arena, BuildError, NodeId};

pub mod model;
pub mod scalar;
pub(crate) mod structural;

/// Racional normalizado de la frontera de construcción, no un nodo ejecutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    numerator: i32,
    denominator: u32,
}

/// Rechazo de metadatos antes de sintetizar; no identifica nodos ejecutables.
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
    structural::exp(arena, x)
}

/// z = Magma(One, exp(exp(One))); conserva la cancelación numérica.
pub fn zero(arena: &mut Arena) -> Result<NodeId, BuildError> {
    structural::zero(arena)
}

/// A(x,y) = exp(x) - (P(x)-y), sin restricción de signo en reales.
pub fn add(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    structural::add(arena, x, y)
}

/// N(x) = 1-(1+x).
pub fn neg(arena: &mut Arena, x: &NodeId) -> Result<NodeId, BuildError> {
    structural::neg(arena, x)
}

pub fn sub(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    structural::sub(arena, x, y)
}

/// Producto por descomposición en diferencias de términos positivos.
pub fn mul(arena: &mut Arena, x: &NodeId, y: &NodeId) -> Result<NodeId, BuildError> {
    structural::mul(arena, x, y)
}

/// Sintetiza el racional normalizado usando unidades, suma, logaritmo y negación.
pub fn rational(arena: &mut Arena, value: Rational) -> Result<NodeId, BuildError> {
    structural::rational(arena, value)
}
