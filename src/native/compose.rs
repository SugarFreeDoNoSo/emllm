use super::{BaseRef, NativeError, NativeErrorKind, NativeLimits, Phase, PreparedBase, Resource};
use crate::{model_io::F32Bits, program::Node, synth::structural::Builder};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct NativeRef {
    identity: Arc<()>,
    index: usize,
}
impl NativeRef {
    pub fn index(&self) -> usize {
        self.index
    }
}
impl PartialEq for NativeRef {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.identity, &other.identity)
    }
}
impl Eq for NativeRef {}

#[derive(Debug)]
enum Source {
    Base(Arc<PreparedBase>),
    Output(Arc<NativeOutput>),
}

impl Source {
    fn count(&self) -> usize {
        match self {
            Self::Base(base) => base.program().node_count(),
            Self::Output(output) => output.nodes,
        }
    }
    fn base(&self) -> &Arc<PreparedBase> {
        let mut source = self;
        loop {
            match source {
                Self::Base(base) => return base,
                Self::Output(output) => source = &output.source,
            }
        }
    }
    fn node(&self, index: usize) -> Option<Node> {
        let mut source = self;
        loop {
            match source {
                Self::Base(base) => return base.program().node(index),
                Self::Output(output) => {
                    let offset = output.nodes - output.pairs.len();
                    if index >= offset {
                        return output
                            .pairs
                            .get(index - offset)
                            .map(|&[a, b]| Node::Magma(a, b));
                    }
                    source = &output.source;
                }
            }
        }
    }
    fn find_pair(&self, pair: [usize; 2]) -> Option<usize> {
        let mut source = self;
        loop {
            match source {
                Self::Base(base) => return base.find_pair_indices(pair),
                Self::Output(output) => {
                    if let Some(row) = output.pairs.iter().position(|p| *p == pair) {
                        return Some(output.nodes - output.pairs.len() + row);
                    }
                    source = &output.source;
                }
            }
        }
    }
}

/// Grafo cerrado propietario de su delta y con base/dependencias retenidas.
/// No conserva plantilla, internado mutable ni valores numéricos.
#[derive(Debug)]
pub struct NativeOutput {
    identity: Arc<()>,
    source: Source,
    pairs: Vec<[usize; 2]>,
    roots: Vec<usize>,
    nodes: usize,
    construction_index_capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputStorage {
    pub closed_nodes: usize,
    pub shared_source_nodes: usize,
    pub new_nodes: usize,
    pub roots: usize,
    pub pair_payload_bytes: usize,
    pub pair_capacity_bytes: usize,
    pub root_payload_bytes: usize,
    pub root_capacity_bytes: usize,
    pub metadata_bytes: usize,
    /// Histórico del internado liberado antes de retornar, no almacenamiento vivo.
    pub construction_index_capacity: usize,
    pub construction_index_modeled_payload_bytes: usize,
}

impl NativeOutput {
    pub fn base(&self) -> &Arc<PreparedBase> {
        self.source.base()
    }
    pub fn node_count(&self) -> usize {
        self.nodes
    }
    pub fn node(&self, index: usize) -> Option<Node> {
        let offset = self.nodes - self.pairs.len();
        if index < offset {
            self.source.node(index)
        } else {
            self.pairs
                .get(index - offset)
                .map(|&[a, b]| Node::Magma(a, b))
        }
    }
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Node> + '_ {
        (0..self.nodes).map(|i| self.node(i).expect("closed dense graph"))
    }
    pub fn delta_pairs(&self) -> &[[usize; 2]] {
        &self.pairs
    }
    pub fn root_indices(&self) -> &[usize] {
        &self.roots
    }
    pub fn roots(&self) -> impl ExactSizeIterator<Item = NativeRef> + '_ {
        self.roots.iter().map(|&index| NativeRef {
            identity: self.identity.clone(),
            index,
        })
    }
    pub fn resolve(&self, reference: &NativeRef) -> Result<Node, NativeError> {
        validate(&self.identity, self.nodes, reference)?;
        Ok(self
            .node(reference.index)
            .expect("validated output reference"))
    }
    pub fn storage(&self) -> OutputStorage {
        OutputStorage {
            closed_nodes: self.nodes,
            shared_source_nodes: self.source.count(),
            new_nodes: self.pairs.len(),
            roots: self.roots.len(),
            pair_payload_bytes: size_of_val(self.pairs.as_slice()),
            pair_capacity_bytes: self.pairs.capacity() * size_of::<[usize; 2]>(),
            root_payload_bytes: size_of_val(self.roots.as_slice()),
            root_capacity_bytes: self.roots.capacity() * size_of::<usize>(),
            metadata_bytes: size_of::<Self>(),
            construction_index_capacity: self.construction_index_capacity,
            construction_index_modeled_payload_bytes: self.construction_index_capacity
                * size_of::<([usize; 2], usize)>(),
        }
    }
}

/// Un contexto nuevo por solicitud. `new` solo comparte base; `extend` es una
/// operación explícita de consumo estructural de una salida, no una repetición.
pub struct CompositionBuilder {
    identity: Arc<()>,
    source: Source,
    offset: usize,
    pairs: Vec<[usize; 2]>,
    interned: HashMap<[usize; 2], usize>,
    limits: NativeLimits,
}

impl CompositionBuilder {
    pub fn new(base: Arc<PreparedBase>, limits: NativeLimits) -> Result<Self, NativeError> {
        Self::from_source(Source::Base(base), limits)
    }
    pub fn extend(output: Arc<NativeOutput>, limits: NativeLimits) -> Result<Self, NativeError> {
        Self::from_source(Source::Output(output), limits)
    }
    fn from_source(source: Source, limits: NativeLimits) -> Result<Self, NativeError> {
        let offset = source.count();
        limits.check(Phase::Compose, Resource::ClosedNodes, offset)?;
        Ok(Self {
            identity: Arc::new(()),
            source,
            offset,
            pairs: Vec::new(),
            interned: HashMap::new(),
            limits,
        })
    }
    pub fn base_reference(&self, reference: &BaseRef) -> Result<NativeRef, NativeError> {
        let index = self.source.base().validate(reference).map_err(|mut e| {
            e.phase = Phase::Compose;
            e
        })?;
        Ok(self.handle(index))
    }
    pub fn output_reference(&self, reference: &NativeRef) -> Result<NativeRef, NativeError> {
        let Source::Output(output) = &self.source else {
            return Err(error(NativeErrorKind::ForeignReference));
        };
        let index = validate(&output.identity, output.nodes, reference)?;
        Ok(self.handle(index))
    }
    pub fn one(&mut self) -> Result<NativeRef, NativeError> {
        <Self as Builder>::one(self)
    }
    pub fn magma(&mut self, left: &NativeRef, right: &NativeRef) -> Result<NativeRef, NativeError> {
        <Self as Builder>::magma(self, left, right)
    }
    pub fn node_count(&self) -> usize {
        self.offset + self.pairs.len()
    }
    fn handle(&self, index: usize) -> NativeRef {
        NativeRef {
            identity: self.identity.clone(),
            index,
        }
    }

    pub(crate) fn reference_at(&self, index: usize) -> Result<NativeRef, NativeError> {
        if index >= self.node_count() {
            return Err(error(NativeErrorKind::InvalidReference { index }));
        }
        Ok(self.handle(index))
    }

    pub fn finish(self, roots: &[NativeRef]) -> Result<NativeOutput, NativeError> {
        if roots.is_empty() {
            return Err(error(NativeErrorKind::EmptyRoots));
        }
        let mut indices = Vec::new();
        self.limits
            .reserve(Phase::Compose, Resource::Roots, &mut indices, roots.len())?;
        for root in roots {
            indices.push(validate(&self.identity, self.node_count(), root)?);
        }
        let nodes = self.node_count();
        let index_capacity = self.interned.capacity();
        // self.interned y todos los temporales internos se liberan antes de retornar.
        Ok(NativeOutput {
            identity: Arc::new(()),
            source: self.source,
            pairs: self.pairs,
            roots: indices,
            nodes,
            construction_index_capacity: index_capacity,
        })
    }
}

impl Builder for CompositionBuilder {
    type Ref = NativeRef;
    type Error = NativeError;
    fn one(&mut self) -> Result<NativeRef, NativeError> {
        Ok(self.handle(0))
    }
    fn constant(&mut self, bits: F32Bits) -> Result<NativeRef, NativeError> {
        let reference = self
            .source
            .base()
            .constant(bits)
            .ok_or_else(|| error(NativeErrorKind::MissingConstant { bits: bits.bits() }))?;
        self.base_reference(&reference)
    }
    fn magma(&mut self, left: &NativeRef, right: &NativeRef) -> Result<NativeRef, NativeError> {
        let pair = [
            validate(&self.identity, self.node_count(), left)?,
            validate(&self.identity, self.node_count(), right)?,
        ];
        if let Some(index) = self.source.find_pair(pair) {
            return Ok(self.handle(index));
        }
        if let Some(&index) = self.interned.get(&pair) {
            return Ok(self.handle(index));
        }
        self.limits
            .checked_add(Phase::Compose, Resource::ClosedNodes, self.node_count(), 1)?;
        self.limits
            .reserve(Phase::Compose, Resource::ClosedNodes, &mut self.pairs, 1)?;
        self.limits
            .reserve_map(Phase::Compose, Resource::ClosedNodes, &mut self.interned, 1)?;
        let index = self.node_count();
        self.pairs.push(pair);
        self.interned.insert(pair, index);
        Ok(self.handle(index))
    }
}

impl super::tensor::TensorBuilder for CompositionBuilder {
    fn phase(&self) -> Phase {
        Phase::Compose
    }
    fn limits(&self) -> NativeLimits {
        self.limits
    }
    fn base(&self) -> &Arc<PreparedBase> {
        self.source.base()
    }
    fn import_base(&self, reference: &BaseRef) -> Result<NativeRef, NativeError> {
        self.base_reference(reference)
    }
    fn validate_reference(&self, reference: &NativeRef) -> Result<(), NativeError> {
        validate(&self.identity, self.node_count(), reference).map(|_| ())
    }
}

fn error(kind: NativeErrorKind) -> NativeError {
    NativeError {
        phase: Phase::Compose,
        kind,
    }
}
fn validate(identity: &Arc<()>, nodes: usize, reference: &NativeRef) -> Result<usize, NativeError> {
    if !Arc::ptr_eq(identity, &reference.identity) {
        return Err(error(NativeErrorKind::ForeignReference));
    }
    if reference.index >= nodes {
        return Err(error(NativeErrorKind::InvalidReference {
            index: reference.index,
        }));
    }
    Ok(reference.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Arc<PreparedBase> {
        Arc::new(
            PreparedBase::from_constants(&[F32Bits::new(0).unwrap()], NativeLimits::default())
                .unwrap(),
        )
    }

    #[test]
    fn source_lives_after_builders_and_can_feed_another_structural_composition() {
        let output = {
            let base = base();
            let mut request =
                CompositionBuilder::new(base.clone(), NativeLimits::default()).unwrap();
            let zero = request
                .base_reference(&base.constant(F32Bits::new(0).unwrap()).unwrap())
                .unwrap();
            let root = request.magma(&zero, &zero).unwrap(); // Fallará numéricamente: aquí solo estructura.
            assert_eq!(root, request.magma(&zero, &zero).unwrap());
            Arc::new(request.finish(&[root]).unwrap())
        };
        let first_nodes = output.node_count();
        let root = output.roots().next().unwrap();
        let mut next = CompositionBuilder::extend(output.clone(), NativeLimits::default()).unwrap();
        let operand = next.output_reference(&root).unwrap();
        drop(output);
        let one = next.one().unwrap();
        let result = next.magma(&operand, &one).unwrap();
        let next = next.finish(&[result]).unwrap();
        assert_eq!(next.node_count(), first_nodes + 1);
        assert_eq!(next.nodes().filter(|n| *n == Node::One).count(), 1);
        for (i, node) in next.nodes().enumerate() {
            if let Node::Magma(a, b) = node {
                assert!(a < i && b < i);
            }
        }
        assert_eq!(next.storage().shared_source_nodes, first_nodes);
        assert_eq!(next.storage().new_nodes, 1);
        assert!(next.resolve(&root).is_err()); // Misma clausura no implica misma identidad.
    }

    #[test]
    fn requests_are_fresh_reject_equal_foreign_indices_and_preserve_base_on_failure() {
        let base = base();
        let before = base.storage();
        let limits = NativeLimits {
            closed_nodes: base.program().node_count() + 1,
            ..NativeLimits::default()
        };
        let mut a = CompositionBuilder::new(base.clone(), limits).unwrap();
        let mut b = CompositionBuilder::new(base.clone(), limits).unwrap();
        let local = a.one().unwrap();
        let foreign = b.one().unwrap();
        assert_eq!(local.index(), foreign.index());
        assert_ne!(local, foreign);
        let zero = a
            .base_reference(&base.constant(F32Bits::new(0).unwrap()).unwrap())
            .unwrap();
        let root = a.magma(&zero, &zero).unwrap();
        assert_eq!(
            a.magma(&local, &foreign).unwrap_err().kind,
            NativeErrorKind::ForeignReference
        );
        assert!(a.magma(&root, &local).is_err());
        let output = a.finish(&[root]).unwrap();
        let zero = b
            .base_reference(&base.constant(F32Bits::new(0).unwrap()).unwrap())
            .unwrap();
        let root = b.magma(&zero, &zero).unwrap();
        let other = b.finish(&[root]).unwrap();
        assert_eq!(output.delta_pairs(), other.delta_pairs());
        assert_ne!(
            output.roots().next().unwrap(),
            other.roots().next().unwrap()
        );
        assert_eq!(base.storage(), before);
        assert!(
            CompositionBuilder::new(
                base,
                NativeLimits {
                    closed_nodes: 0,
                    ..limits
                }
            )
            .is_err()
        );
    }
}
