//! Composición nativa: presupuestos estructurales por fase.
//! La supervisión de RSS/tiempo pertenece a las herramientas externas.

mod codec;
mod compose;
mod decode;
mod instantiate;
mod limits;
mod prepared;
mod template;

pub use codec::{NativePrefix, NativeSymbol};
pub use compose::{CompositionBuilder, NativeOutput, NativeRef, OutputStorage};
pub use decode::{DecodeStats, DecodedOutput, compile_output, decode};
pub use instantiate::{CompositionStats, PreparedModel};
pub use limits::{NativeError, NativeErrorKind, NativeLimits, Phase, Resource};
pub use prepared::{
    BaseRef, BaseStorage, BaseTensor, PreparationError, PreparedBase, SYNTHESIS_VERSION,
};
pub use template::{Template, TemplateBuilder, TemplateRef, TemplateStats};
