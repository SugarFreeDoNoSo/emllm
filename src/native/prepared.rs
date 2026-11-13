//! Base propietaria sin entradas concretas. SHA-256 de fuentes/paquete se verifica
//! externamente antes de llamar a `from_stories260k`, como en `model_io`.

use std::{collections::HashMap, error::Error, fmt, mem::size_of, ops::Range, sync::Arc};

use super::{NativeError, NativeErrorKind, NativeLimits, Phase, Resource};
use crate::{
    model_io::{F32Bits, ModelConfig, Stories260K, TensorKind},
    program::{Node, Program, compile_nodes},
    synth::structural::{self, Builder},
};

pub const SYNTHESIS_VERSION: &str = "one-magma-f32-shifted-log-domain-v1";
const EXPANSION_BITS: [u32; 10] = [
    0,
    0x3f80_0000,
    0xbf80_0000,
    0x4000_0000,
    0x4300_0000,
    0x3c80_0000,
    0x3e00_0000,
    0x4580_0000,
    0x4400_0000,
    0x3f00_0000,
];

#[derive(Clone, Debug)]
pub struct BaseRef {
    identity: Arc<()>,
    index: usize,
}

impl BaseRef {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl PartialEq for BaseRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity) && self.index == other.index
    }
}
impl Eq for BaseRef {}

#[derive(Debug)]
pub struct PreparationError {
    pub tensor: Option<usize>,
    pub scalar: Option<usize>,
    pub bits: Option<u32>,
    pub completed_constants: usize,
    pub constructed_nodes: usize,
    pub cause: NativeError,
}

impl fmt::Display for PreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}; tensor={:?} scalar={:?} bits={:?}; {} constants, {} nodes",
            self.cause,
            self.tensor,
            self.scalar,
            self.bits,
            self.completed_constants,
            self.constructed_nodes
        )
    }
}
impl Error for PreparationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Metadatos propietarios; un alias conserva exactamente el mismo rango de entradas.
#[derive(Debug)]
pub struct BaseTensor {
    kind: TensorKind,
    layer: Option<u32>,
    dimensions: [usize; 2],
    rank: usize,
    alias: Option<usize>,
    entries: Range<usize>,
}

impl BaseTensor {
    pub fn kind(&self) -> TensorKind {
        self.kind
    }
    pub fn layer(&self) -> Option<u32> {
        self.layer
    }
    pub fn shape(&self) -> &[usize] {
        &self.dimensions[..self.rank]
    }
    pub fn alias(&self) -> Option<usize> {
        self.alias
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Base cerrada e inmutable; no retiene arena, paquete, plantilla ni valores evaluados.
/// Las raíces del programa corresponden uno a uno a `constant_bits`, por primera
/// aparición. El índice de pares se reconstruye después del remapeo de compilación.
#[derive(Debug)]
pub struct PreparedBase {
    identity: Arc<()>,
    program: Program,
    constant_bits: Vec<F32Bits>,
    constants: HashMap<F32Bits, usize>,
    pair_index: HashMap<[usize; 2], usize>,
    tensors: Vec<BaseTensor>,
    tensor_entries: Vec<usize>,
    vocabulary: Vec<usize>,
    config: Option<ModelConfig>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseStorage {
    pub nodes: usize,
    pub constants: usize,
    pub tensor_entries: usize,
    pub vocabulary_symbols: usize,
    pub vector_payload_bytes: usize,
    pub vector_capacity_bytes: usize,
    pub pair_index_len: usize,
    pub pair_index_capacity: usize,
    pub pair_index_modeled_payload_bytes: usize,
    pub constant_index_len: usize,
    pub constant_index_capacity: usize,
    pub constant_index_modeled_payload_bytes: usize,
    pub inline_metadata_bytes: usize,
}

impl PreparedBase {
    /// Base pequeña para composiciones y pruebas. No representa stories260K ni
    /// incluye vocabulario/auxiliares de modelo; no admite una lista vacía.
    pub fn from_constants(
        values: &[F32Bits],
        limits: NativeLimits,
    ) -> Result<Self, PreparationError> {
        let mut builder = BaseBuilder::new(limits);
        if values.is_empty() {
            return Err(builder.failure(
                NativeError {
                    phase: Phase::Base,
                    kind: NativeErrorKind::EmptyConstants,
                },
                None,
                None,
                None,
            ));
        }
        for (index, &value) in values.iter().enumerate() {
            builder
                .register(value)
                .map_err(|e| builder.failure(e, None, Some(index), Some(value)))?;
        }
        builder.finish(Vec::new(), Vec::new(), Vec::new(), None)
    }

    /// Consume solo modelo y presupuesto: no recibe IDs ni referencias numéricas.
    /// Los hashes de contenido se comprueban con la herramienta de preparación.
    pub fn from_stories260k(
        model: &Stories260K<'_>,
        limits: NativeLimits,
    ) -> Result<Self, PreparationError> {
        let mut builder = BaseBuilder::new(limits);
        let mut tensors: Vec<BaseTensor> = Vec::new();
        let mut entries = Vec::new();
        for (i, tensor) in model.package().tensors().iter().enumerate() {
            limits
                .reserve(Phase::Base, Resource::Tensors, &mut tensors, 1)
                .map_err(|e| builder.failure(e, Some(i), None, None))?;
            let range = if let Some(owner) = tensor.alias() {
                tensors[owner].entries.clone()
            } else {
                let start = entries.len();
                limits
                    .reserve(
                        Phase::Base,
                        Resource::Roots,
                        &mut entries,
                        tensor.scalars().len(),
                    )
                    .map_err(|e| builder.failure(e, Some(i), None, None))?;
                for (index, value) in tensor.scalars().enumerate() {
                    let record = builder
                        .register(value)
                        .map_err(|e| builder.failure(e, Some(i), Some(index), Some(value)))?;
                    entries.push(record);
                }
                start..entries.len()
            };
            let mut dimensions = [0; 2];
            for (dimension, &value) in dimensions.iter_mut().zip(tensor.shape()) {
                // read_stories260k ya valida estas dimensiones pequeñas y fijas.
                *dimension = value as usize;
            }
            tensors.push(BaseTensor {
                kind: tensor.kind(),
                layer: tensor.layer(),
                dimensions,
                rank: tensor.shape().len(),
                alias: tensor.alias(),
                entries: range,
            });
        }
        for bits in EXPANSION_BITS {
            let value = F32Bits::new(bits).expect("finite fixed metadata");
            builder
                .register(value)
                .map_err(|e| builder.failure(e, None, None, Some(value)))?;
        }
        let mut vocabulary = Vec::new();
        limits
            .reserve(Phase::Base, Resource::Roots, &mut vocabulary, 512)
            .map_err(|e| builder.failure(e, None, None, None))?;
        for id in 0..512 {
            let value = vocabulary_bits(id);
            vocabulary.push(
                builder
                    .register(value)
                    .map_err(|e| builder.failure(e, None, Some(id as usize), Some(value)))?,
            );
        }
        builder.finish(
            tensors,
            entries,
            vocabulary,
            Some(*model.package().config()),
        )
    }

    pub fn program(&self) -> &Program {
        &self.program
    }
    pub fn constant_bits(&self) -> &[F32Bits] {
        &self.constant_bits
    }
    pub fn config(&self) -> Option<ModelConfig> {
        self.config
    }
    pub fn tensors(&self) -> &[BaseTensor] {
        &self.tensors
    }

    pub fn constant(&self, bits: F32Bits) -> Option<BaseRef> {
        self.constants
            .get(&bits)
            .map(|&record| self.handle(self.program.roots()[record]))
    }

    pub fn vocabulary_symbol(&self, id: usize) -> Option<BaseRef> {
        self.vocabulary
            .get(id)
            .map(|&record| self.handle(self.program.roots()[record]))
    }

    /// Acceso por directorio/coordenada plana; los alias seleccionan la misma raíz.
    pub fn tensor_scalar(&self, tensor: usize, index: usize) -> Option<(F32Bits, BaseRef)> {
        let tensor = self.tensors.get(tensor)?;
        if index >= tensor.len() {
            return None;
        }
        let record = self.tensor_entries[tensor.entries.start + index];
        Some((
            self.constant_bits[record],
            self.handle(self.program.roots()[record]),
        ))
    }

    pub fn node(&self, reference: &BaseRef) -> Result<Node, NativeError> {
        let index = self.validate(reference)?;
        Ok(self.program.node(index).expect("validated base reference"))
    }

    pub fn find_pair(
        &self,
        left: &BaseRef,
        right: &BaseRef,
    ) -> Result<Option<BaseRef>, NativeError> {
        let pair = [self.validate(left)?, self.validate(right)?];
        Ok(self.pair_index.get(&pair).map(|&index| self.handle(index)))
    }

    pub(crate) fn validate(&self, reference: &BaseRef) -> Result<usize, NativeError> {
        if !Arc::ptr_eq(&self.identity, &reference.identity) {
            return Err(NativeError {
                phase: Phase::Base,
                kind: NativeErrorKind::ForeignReference,
            });
        }
        if reference.index >= self.program.node_count() {
            return Err(NativeError {
                phase: Phase::Base,
                kind: NativeErrorKind::InvalidReference {
                    index: reference.index,
                },
            });
        }
        Ok(reference.index)
    }

    pub(crate) fn handle(&self, index: usize) -> BaseRef {
        BaseRef {
            identity: self.identity.clone(),
            index,
        }
    }

    /// Mapas: solo payload modelado de capacidad; excluye control y asignador.
    /// La identidad Arc y su bloque de control tampoco son bytes observables de std.
    pub fn storage(&self) -> BaseStorage {
        let program = self.program.storage();
        BaseStorage {
            nodes: self.program.node_count(),
            constants: self.constant_bits.len(),
            tensor_entries: self.tensor_entries.len(),
            vocabulary_symbols: self.vocabulary.len(),
            vector_payload_bytes: program.pair_payload_bytes
                + program.root_payload_bytes
                + size_of_val(self.constant_bits.as_slice())
                + size_of_val(self.tensor_entries.as_slice())
                + size_of_val(self.vocabulary.as_slice())
                + size_of_val(self.tensors.as_slice()),
            vector_capacity_bytes: program.pair_capacity_bytes
                + program.root_capacity_bytes
                + self.constant_bits.capacity() * size_of::<F32Bits>()
                + self.tensor_entries.capacity() * size_of::<usize>()
                + self.vocabulary.capacity() * size_of::<usize>()
                + self.tensors.capacity() * size_of::<BaseTensor>(),
            pair_index_len: self.pair_index.len(),
            pair_index_capacity: self.pair_index.capacity(),
            pair_index_modeled_payload_bytes: self.pair_index.capacity()
                * size_of::<([usize; 2], usize)>(),
            constant_index_len: self.constants.len(),
            constant_index_capacity: self.constants.capacity(),
            constant_index_modeled_payload_bytes: self.constants.capacity()
                * size_of::<(F32Bits, usize)>(),
            inline_metadata_bytes: size_of::<Self>(),
        }
    }
}

// IDs enteros exactos 0..511 a bits IEEE, sin ejecutar aritmética del modelo.
fn vocabulary_bits(id: u32) -> F32Bits {
    let bits = if id == 0 {
        0
    } else {
        let exponent = 31 - id.leading_zeros();
        ((exponent + 127) << 23) | ((id - (1 << exponent)) << (23 - exponent))
    };
    F32Bits::new(bits).expect("finite vocabulary metadata")
}

struct BaseBuilder {
    identity: Arc<()>,
    pairs: Vec<[usize; 2]>,
    has_one: bool,
    interned: HashMap<[usize; 2], usize>,
    limits: NativeLimits,
    bits: Vec<F32Bits>,
    roots: Vec<usize>,
    constants: HashMap<F32Bits, usize>,
}

impl BaseBuilder {
    fn new(limits: NativeLimits) -> Self {
        Self {
            identity: Arc::new(()),
            pairs: Vec::new(),
            has_one: false,
            interned: HashMap::new(),
            limits,
            bits: Vec::new(),
            roots: Vec::new(),
            constants: HashMap::new(),
        }
    }
    fn nodes(&self) -> usize {
        self.pairs.len() + usize::from(self.has_one)
    }
    fn failure(
        &self,
        cause: NativeError,
        tensor: Option<usize>,
        scalar: Option<usize>,
        bits: Option<F32Bits>,
    ) -> PreparationError {
        PreparationError {
            tensor,
            scalar,
            bits: bits.map(F32Bits::bits),
            completed_constants: self.roots.len(),
            constructed_nodes: self.nodes(),
            cause,
        }
    }
    fn handle(&self, index: usize) -> BaseRef {
        BaseRef {
            identity: self.identity.clone(),
            index,
        }
    }
    fn validate(&self, reference: &BaseRef) -> Result<usize, NativeError> {
        if !Arc::ptr_eq(&self.identity, &reference.identity) {
            return Err(NativeError {
                phase: Phase::Base,
                kind: NativeErrorKind::ForeignReference,
            });
        }
        if reference.index >= self.nodes() {
            return Err(NativeError {
                phase: Phase::Base,
                kind: NativeErrorKind::InvalidReference {
                    index: reference.index,
                },
            });
        }
        Ok(reference.index)
    }
    fn register(&mut self, value: F32Bits) -> Result<usize, NativeError> {
        if let Some(&record) = self.constants.get(&value) {
            return Ok(record);
        }
        self.limits
            .reserve(Phase::Base, Resource::Roots, &mut self.roots, 1)?;
        self.limits
            .reserve(Phase::Base, Resource::Roots, &mut self.bits, 1)?;
        self.limits
            .reserve_map(Phase::Base, Resource::Roots, &mut self.constants, 1)?;
        let root = structural::scalar::synthesize(self, value)?;
        let record = self.roots.len();
        self.roots.push(root.index);
        self.bits.push(value);
        self.constants.insert(value, record);
        Ok(record)
    }
    fn finish(
        mut self,
        tensors: Vec<BaseTensor>,
        tensor_entries: Vec<usize>,
        vocabulary: Vec<usize>,
        config: Option<ModelConfig>,
    ) -> Result<PreparedBase, PreparationError> {
        // El mapa de construcción ya no hace falta durante la compilación.
        self.interned = HashMap::new();
        let program = compile_nodes(
            |index| {
                if index == 0 {
                    return self.has_one.then_some(Node::One);
                }
                self.pairs.get(index - 1).map(|&[a, b]| Node::Magma(a, b))
            },
            &self.roots,
            self.limits.base_nodes,
        )
        .map_err(|error| {
            self.failure(
                NativeError {
                    phase: Phase::Base,
                    kind: NativeErrorKind::Compile(error),
                },
                None,
                None,
                None,
            )
        })?;
        let constructed_nodes = self.nodes();
        drop(self.pairs);
        drop(self.roots);
        let mut pair_index = HashMap::new();
        self.limits
            .reserve_map(
                Phase::Base,
                Resource::BaseNodes,
                &mut pair_index,
                program.pairs().len(),
            )
            .map_err(|cause| PreparationError {
                tensor: None,
                scalar: None,
                bits: None,
                completed_constants: self.bits.len(),
                constructed_nodes,
                cause,
            })?;
        for (row, &pair) in program.pairs().iter().enumerate() {
            pair_index.insert(pair, row + 1);
        }
        Ok(PreparedBase {
            identity: Arc::new(()),
            program,
            constant_bits: self.bits,
            constants: self.constants,
            pair_index,
            tensors,
            tensor_entries,
            vocabulary,
            config,
        })
    }
}

impl Builder for BaseBuilder {
    type Ref = BaseRef;
    type Error = NativeError;
    fn one(&mut self) -> Result<BaseRef, NativeError> {
        if !self.has_one {
            self.limits.check(Phase::Base, Resource::BaseNodes, 1)?;
            self.has_one = true;
        }
        Ok(self.handle(0))
    }
    fn magma(&mut self, left: &BaseRef, right: &BaseRef) -> Result<BaseRef, NativeError> {
        let pair = [self.validate(left)?, self.validate(right)?];
        if let Some(&index) = self.interned.get(&pair) {
            return Ok(self.handle(index));
        }
        self.limits
            .checked_add(Phase::Base, Resource::BaseNodes, self.nodes(), 1)?;
        self.limits
            .reserve(Phase::Base, Resource::BaseNodes, &mut self.pairs, 1)?;
        self.limits
            .reserve_map(Phase::Base, Resource::BaseNodes, &mut self.interned, 1)?;
        let index = self.nodes();
        self.pairs.push(pair);
        self.interned.insert(pair, index);
        Ok(self.handle(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        eval::evaluate,
        execution::{PlanLimits, plan, run},
    };

    #[test]
    fn constants_own_their_graph_and_both_readers_agree() {
        let bits = [0, 0x8000_0000, 0x3d14_ebf3, 0xbf00_0000, 0x3d14_ebf3];
        let base = {
            let source: Vec<_> = bits.into_iter().map(|b| F32Bits::new(b).unwrap()).collect();
            PreparedBase::from_constants(&source, NativeLimits::default()).unwrap()
        };
        assert_eq!(base.constant_bits.len(), 4);
        let values = evaluate(base.program(), 10000).unwrap();
        let plan = plan(
            base.program(),
            PlanLimits {
                max_nodes: 10000,
                max_slots: 10000,
            },
        )
        .unwrap();
        assert_eq!(values, run(&plan, 10000).unwrap());
        for (bits, value) in base.constant_bits().iter().zip(values) {
            assert!((value - bits.to_f64()).abs() <= 1e-9 + 1e-7 * bits.to_f64().abs());
        }
        assert_eq!(
            base.constant(F32Bits::new(0).unwrap()),
            base.constant(F32Bits::new(0x8000_0000).unwrap())
        );
        let storage = base.storage();
        assert_eq!(storage.pair_index_len, base.program().pairs().len());
        assert!(storage.vector_capacity_bytes >= storage.vector_payload_bytes);
    }

    #[test]
    fn limits_and_foreign_references_preserve_existing_structure() {
        let value = F32Bits::new(0x3d14_ebf3).unwrap();
        let limits = NativeLimits::default();
        let base = PreparedBase::from_constants(&[value], limits).unwrap();
        let nodes = base.program().node_count();
        PreparedBase::from_constants(
            &[value],
            NativeLimits {
                base_nodes: nodes,
                ..limits
            },
        )
        .unwrap();
        let failure = PreparedBase::from_constants(
            &[value],
            NativeLimits {
                base_nodes: nodes - 1,
                ..limits
            },
        )
        .unwrap_err();
        assert_eq!(failure.bits, Some(value.bits()));
        assert_eq!(failure.completed_constants, 0);
        assert!(matches!(
            failure.cause.kind,
            NativeErrorKind::Limit {
                resource: Resource::BaseNodes,
                ..
            }
        ));
        let other = PreparedBase::from_constants(&[value], limits).unwrap();
        let local = base.constant(value).unwrap();
        let foreign = other.constant(value).unwrap();
        assert_eq!(local.index(), foreign.index());
        assert_ne!(local, foreign);
        assert_eq!(
            base.node(&foreign).unwrap_err().kind,
            NativeErrorKind::ForeignReference
        );
        assert_eq!(
            base.find_pair(&local, &foreign).unwrap_err().kind,
            NativeErrorKind::ForeignReference
        );
        let mut a = BaseBuilder::new(limits);
        let mut b = BaseBuilder::new(limits);
        let one = a.one().unwrap();
        assert!(a.magma(&one, &b.one().unwrap()).is_err());
        assert_eq!(a.nodes(), 1);
    }

    #[test]
    fn vocabulary_bits_and_aliases_keep_every_coordinate() {
        for id in 0..512 {
            assert_eq!(vocabulary_bits(id).to_f64(), f64::from(id));
        }
        let value = F32Bits::new(0x3f00_0000).unwrap();
        let mut builder = BaseBuilder::new(NativeLimits::default());
        let record = builder.register(value).unwrap();
        let tensors = vec![
            BaseTensor {
                kind: TensorKind::Embedding,
                layer: None,
                dimensions: [1, 1],
                rank: 2,
                alias: None,
                entries: 0..1,
            },
            BaseTensor {
                kind: TensorKind::Output,
                layer: None,
                dimensions: [1, 1],
                rank: 2,
                alias: Some(0),
                entries: 0..1,
            },
        ];
        let base = builder.finish(tensors, vec![record], vec![], None).unwrap();
        assert_eq!(base.tensor_scalar(0, 0), base.tensor_scalar(1, 0));
        assert_eq!(base.storage().tensor_entries, 1);
        assert!(base.tensor_scalar(1, 1).is_none());
    }
}
