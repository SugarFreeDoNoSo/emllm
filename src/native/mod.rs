//! Composición nativa: presupuestos estructurales por fase.
//! La supervisión de RSS/tiempo pertenece a las herramientas externas.

mod limits;
mod prepared;
mod template;

pub use limits::{NativeError, NativeErrorKind, NativeLimits, Phase, Resource};
pub use prepared::{
    BaseRef, BaseStorage, BaseTensor, PreparationError, PreparedBase, SYNTHESIS_VERSION,
};
pub use template::{Template, TemplateBuilder, TemplateRef, TemplateStats};
