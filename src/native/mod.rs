//! Composición nativa: presupuestos estructurales por fase.
//! La supervisión de RSS/tiempo pertenece a las herramientas externas.

mod limits;

pub use limits::{NativeError, NativeErrorKind, NativeLimits, Phase, Resource};
