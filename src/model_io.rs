//! Lector EMLTINY v1: valida datos F32, sin construir ni evaluar nodos.
//!
//! [`read_package`] comprueba el contenedor; [`read_stories260k`] exige además
//! configuración, inventario, formas, alias y constantes del modelo objetivo.
//! Ambos toman límites explícitos y prestan los bytes inmutables del llamador.
//! SHA-256 y procedencia se verifican en las herramientas externas.

use std::{error::Error, fmt};

const HEADER: usize = 64;
const RECORD: usize = 48;
const NONE: u32 = u32::MAX;

/// Límites de entrada, comprobados antes de reservar el directorio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadLimits {
    pub max_bytes: usize,
    pub max_tensors: usize,
}

/// Bits de un F32 finito; no es una hoja del lenguaje ejecutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct F32Bits(u32);

impl F32Bits {
    /// Conserva incluso el signo de cero; rechaza todos los NaN e infinitos.
    pub fn new(bits: u32) -> Option<Self> {
        (bits & 0x7f80_0000 != 0x7f80_0000).then_some(Self(bits))
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    /// Elevación exacta del F32, útil como oráculo externo a la construcción.
    pub fn to_f64(self) -> f64 {
        f64::from(f32::from_bits(self.0))
    }
}

/// Catálogo v1. Solo los tipos 2..10 llevan un índice de capa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TensorKind {
    Embedding = 1,
    AttentionNorm,
    Query,
    Key,
    Value,
    AttentionOutput,
    FeedForwardNorm,
    Gate,
    Down,
    Up,
    FinalNorm,
    Output,
    RopeCos,
    RopeSin,
    Epsilon,
    AttentionScale,
}

impl TensorKind {
    fn from_code(code: u32) -> Option<Self> {
        use TensorKind::*;
        Some(match code {
            1 => Embedding,
            2 => AttentionNorm,
            3 => Query,
            4 => Key,
            5 => Value,
            6 => AttentionOutput,
            7 => FeedForwardNorm,
            8 => Gate,
            9 => Down,
            10 => Up,
            11 => FinalNorm,
            12 => Output,
            13 => RopeCos,
            14 => RopeSin,
            15 => Epsilon,
            16 => AttentionScale,
            _ => return None,
        })
    }

    fn is_layer(self) -> bool {
        (2..=10).contains(&(self as u32))
    }
}

/// Copia de la configuración entera validada. No concede acceso mutable al paquete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelConfig {
    pub dim: u32,
    pub hidden_dim: u32,
    pub n_layers: u32,
    pub n_heads: u32,
    pub n_kv_heads: u32,
    pub vocab_size: u32,
    pub max_seq_len: u32,
    pub rope_positions: u32,
}

/// Entrada inmutable validada. Los alias prestan exactamente los mismos bytes.
#[derive(Debug)]
pub struct Tensor<'a> {
    kind: TensorKind,
    layer: Option<u32>,
    rank: usize,
    dimensions: [u64; 2],
    alias: Option<usize>,
    offset: usize,
    data: &'a [u8],
}

impl Tensor<'_> {
    pub fn kind(&self) -> TensorKind {
        self.kind
    }

    pub fn layer(&self) -> Option<u32> {
        self.layer
    }

    pub fn shape(&self) -> &[u64] {
        &self.dimensions[..self.rank]
    }

    /// Índice de la entrada propietaria, o `None` si esta entrada posee el payload.
    pub fn alias(&self) -> Option<usize> {
        self.alias
    }

    pub fn byte_offset(&self) -> usize {
        self.offset
    }

    /// Payload F32 little-endian, compartido por alias y preservado por bits.
    pub fn bytes(&self) -> &[u8] {
        self.data
    }

    pub fn scalars(&self) -> impl ExactSizeIterator<Item = F32Bits> + '_ {
        self.data
            .chunks_exact(4)
            .map(|chunk| F32Bits(u32::from_le_bytes(chunk.try_into().expect("validated F32"))))
    }
}

/// Contenedor validado, aún no necesariamente compatible con stories260K.
#[derive(Debug)]
pub struct Package<'a> {
    config: ModelConfig,
    tensors: Vec<Tensor<'a>>,
}

impl Package<'_> {
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }

    pub fn tensors(&self) -> &[Tensor<'_>] {
        &self.tensors
    }
}

/// Modelo con configuración e inventario objetivo; solo el lector puede crearlo.
///
/// ```compile_fail
/// use emllm::model_io::Stories260K;
/// let model = Stories260K { package: todo!() };
/// ```
#[derive(Debug)]
pub struct Stories260K<'a> {
    package: Package<'a>,
}

impl Stories260K<'_> {
    pub fn package(&self) -> &Package<'_> {
        &self.package
    }

    pub fn tensor(&self, kind: TensorKind, layer: Option<u32>) -> Option<&Tensor<'_>> {
        self.package
            .tensors
            .iter()
            .find(|tensor| tensor.kind == kind && tensor.layer == layer)
    }
}

/// Categoría estable para distinguir fallos de carga antes de construir nodos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadErrorKind {
    ByteLimit,
    TensorLimit,
    Truncated,
    Magic,
    Version,
    Dimension,
    Layout,
    TensorKind,
    DuplicateTensor,
    Shape,
    SizeOverflow,
    Alias,
    NonFinite,
    Configuration,
    Inventory,
    Constant,
    Allocation,
}

/// No entrega un paquete parcial. `tensor` es el índice del directorio, si existe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadError {
    pub kind: LoadErrorKind,
    pub tensor: Option<usize>,
    pub field: &'static str,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} in {}", self.kind, self.field)?;
        if let Some(index) = self.tensor {
            write!(f, " at tensor {index}")?;
        }
        Ok(())
    }
}

impl Error for LoadError {}

fn error(kind: LoadErrorKind, tensor: Option<usize>, field: &'static str) -> LoadError {
    LoadError {
        kind,
        tensor,
        field,
    }
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().expect("checked record"))
}

fn u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().expect("checked record"))
}

/// Valida estructura, límites y todos los escalares antes de devolver el paquete.
/// No copia el payload ni reserva almacenamiento en función de formas sin validar.
pub fn read_package(data: &[u8], limits: LoadLimits) -> Result<Package<'_>, LoadError> {
    use LoadErrorKind as K;
    let header_error = |kind, field| error(kind, None, field);
    if data.len() > limits.max_bytes {
        return Err(header_error(K::ByteLimit, "file"));
    }
    if data.len() < HEADER {
        return Err(header_error(K::Truncated, "header"));
    }
    if &data[..8] != b"EMLTINY\0" {
        return Err(header_error(K::Magic, "magic"));
    }
    if u32_at(data, 8) != 1 {
        return Err(header_error(K::Version, "version"));
    }
    let count = usize::try_from(u32_at(data, 12))
        .map_err(|_| header_error(K::SizeOverflow, "tensor_count"))?;
    if count == 0 || count > limits.max_tensors {
        return Err(header_error(K::TensorLimit, "tensor_count"));
    }
    let fields = [
        "dim",
        "hidden_dim",
        "n_layers",
        "n_heads",
        "n_kv_heads",
        "vocab_size",
        "max_seq_len",
        "rope_positions",
    ];
    for (index, field) in fields.iter().enumerate() {
        if u32_at(data, 16 + 4 * index) == 0 {
            return Err(header_error(K::Dimension, field));
        }
    }
    let config = ModelConfig {
        dim: u32_at(data, 16),
        hidden_dim: u32_at(data, 20),
        n_layers: u32_at(data, 24),
        n_heads: u32_at(data, 28),
        n_kv_heads: u32_at(data, 32),
        vocab_size: u32_at(data, 36),
        max_seq_len: u32_at(data, 40),
        rope_positions: u32_at(data, 44),
    };
    let payload = count
        .checked_mul(RECORD)
        .and_then(|n| n.checked_add(HEADER))
        .ok_or_else(|| header_error(K::SizeOverflow, "directory"))?;
    if payload > limits.max_bytes || u64_at(data, 56) > limits.max_bytes as u64 {
        return Err(header_error(K::ByteLimit, "directory"));
    }
    if u64_at(data, 48) != HEADER as u64 || u64_at(data, 56) != payload as u64 {
        return Err(header_error(K::Layout, "directory"));
    }
    if payload > data.len() {
        return Err(header_error(K::Truncated, "directory"));
    }
    let mut tensors: Vec<Tensor<'_>> = Vec::new();
    tensors
        .try_reserve_exact(count)
        .map_err(|_| header_error(K::Allocation, "directory"))?;
    let mut cursor = payload;
    for index in 0..count {
        let fail = |kind, field| error(kind, Some(index), field);
        let record = &data[HEADER + index * RECORD..HEADER + (index + 1) * RECORD];
        let kind =
            TensorKind::from_code(u32_at(record, 0)).ok_or_else(|| fail(K::TensorKind, "kind"))?;
        let raw_layer = u32_at(record, 4);
        if (kind.is_layer() && raw_layer >= config.n_layers)
            || (!kind.is_layer() && raw_layer != NONE)
        {
            return Err(fail(K::TensorKind, "layer"));
        }
        let layer = (raw_layer != NONE).then_some(raw_layer);
        if tensors.iter().any(|t| t.kind == kind && t.layer == layer) {
            return Err(fail(K::DuplicateTensor, "kind/layer"));
        }
        let rank = u32_at(record, 8) as usize;
        let dimensions = [u64_at(record, 16), u64_at(record, 24)];
        if rank > 2 || dimensions[rank..].iter().any(|&n| n != 1) || dimensions[..rank].contains(&0)
        {
            return Err(fail(K::Shape, "dimensions"));
        }
        let size = dimensions[..rank]
            .iter()
            .try_fold(4_u64, |n, &d| n.checked_mul(d))
            .ok_or_else(|| fail(K::SizeOverflow, "dimensions"))?;
        let offset = u64_at(record, 32);
        let declared_size = u64_at(record, 40);
        let end = offset
            .checked_add(declared_size)
            .ok_or_else(|| fail(K::SizeOverflow, "offset/size"))?;
        if size > limits.max_bytes as u64 || end > limits.max_bytes as u64 {
            return Err(fail(K::ByteLimit, "payload"));
        }
        if size != declared_size {
            return Err(fail(K::Shape, "size"));
        }
        let offset = usize::try_from(offset).map_err(|_| fail(K::SizeOverflow, "offset"))?;
        let end = usize::try_from(end).map_err(|_| fail(K::SizeOverflow, "end"))?;
        let raw_alias = u32_at(record, 12);
        let alias = if raw_alias == NONE {
            None
        } else {
            Some(usize::try_from(raw_alias).map_err(|_| fail(K::Alias, "target"))?)
        };
        let body = if let Some(target) = alias {
            let target = tensors
                .get(target)
                .ok_or_else(|| fail(K::Alias, "target"))?;
            if target.alias.is_some()
                || target.shape() != &dimensions[..rank]
                || target.offset != offset
                || target.data.len() as u64 != size
            {
                return Err(fail(K::Alias, "layout"));
            }
            target.data
        } else {
            if offset != cursor {
                return Err(fail(K::Layout, "offset"));
            }
            if end > data.len() {
                return Err(fail(K::Truncated, "payload"));
            }
            let body = &data[offset..end];
            for chunk in body.chunks_exact(4) {
                if F32Bits::new(u32_at(chunk, 0)).is_none() {
                    return Err(fail(K::NonFinite, "scalar"));
                }
            }
            cursor = end;
            body
        };
        tensors.push(Tensor {
            kind,
            layer,
            rank,
            dimensions,
            alias,
            offset,
            data: body,
        });
    }
    if cursor != data.len() {
        return Err(header_error(K::Layout, "trailing_bytes"));
    }
    Ok(Package { config, tensors })
}

/// Exige stories260K canónico: cinco bloques, GQA 8/4, MLP 172 y RoPE para 16
/// posiciones. La identidad criptográfica de los pesos se verifica externamente.
pub fn read_stories260k(data: &[u8], limits: LoadLimits) -> Result<Stories260K<'_>, LoadError> {
    use LoadErrorKind as K;
    use TensorKind::*;
    let package = read_package(data, limits)?;
    let c = package.config;
    for (field, actual, expected) in [
        ("dim", c.dim, 64),
        ("hidden_dim", c.hidden_dim, 172),
        ("n_layers", c.n_layers, 5),
        ("n_heads", c.n_heads, 8),
        ("n_kv_heads", c.n_kv_heads, 4),
        ("vocab_size", c.vocab_size, 512),
        ("max_seq_len", c.max_seq_len, 512),
        ("rope_positions", c.rope_positions, 16),
    ] {
        if actual != expected {
            return Err(error(K::Configuration, None, field));
        }
    }
    if package.tensors.len() != 52 {
        return Err(error(K::Inventory, None, "tensor_count"));
    }
    for (index, tensor) in package.tensors.iter().enumerate() {
        let (kind_code, layer) = match index {
            0 => (1, None),
            1..=45 => (2 + (index - 1) as u32 % 9, Some((index - 1) as u32 / 9)),
            _ => (11 + (index - 46) as u32, None),
        };
        if tensor.kind as u32 != kind_code || tensor.layer != layer {
            return Err(error(K::Inventory, Some(index), "kind/layer/order"));
        }
        let shape: &[u64] = match tensor.kind {
            Embedding | Output => &[512, 64],
            AttentionNorm | FeedForwardNorm | FinalNorm => &[64],
            Query | AttentionOutput => &[64, 64],
            Key | Value => &[32, 64],
            Gate | Up => &[172, 64],
            Down => &[64, 172],
            RopeCos | RopeSin => &[16, 4],
            Epsilon | AttentionScale => &[],
        };
        if tensor.shape() != shape {
            return Err(error(K::Shape, Some(index), "model_shape"));
        }
        if tensor.alias != (tensor.kind == Output).then_some(0) {
            return Err(error(K::Alias, Some(index), "model_alias"));
        }
        let constant = match tensor.kind {
            Epsilon => Some(0x3727_c5ac),
            AttentionScale => Some(0x3eb5_04f3),
            _ => None,
        };
        if constant.is_some_and(|bits| u32_at(tensor.data, 0) != bits) {
            return Err(error(K::Constant, Some(index), "model_constant"));
        }
    }
    Ok(Stories260K { package })
}
