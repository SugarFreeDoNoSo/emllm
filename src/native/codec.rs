//! Codificación estructural; no accede a embeddings ni evalúa símbolos.
use super::{BaseRef, NativeError, NativeErrorKind, NativeLimits, Phase, PreparedBase, Resource};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeSymbol {
    id: u32,
    root: BaseRef,
}

impl NativeSymbol {
    pub fn registered(base: &PreparedBase, id: u32) -> Result<Self, NativeError> {
        let root = base
            .vocabulary_symbol(id as usize)
            .ok_or_else(|| error(NativeErrorKind::TokenRange { id }))?;
        Ok(Self { id, root })
    }
    /// Metadata privada registrada, no un ID deducido mediante evaluación de root.
    pub fn token_id(&self) -> u32 {
        self.id
    }
    pub fn root(&self) -> &BaseRef {
        &self.root
    }
}

/// Entrada codificada reutilizable: símbolos/posiciones, sin activaciones ni puertos.
#[derive(Debug)]
pub struct NativePrefix {
    base: Arc<PreparedBase>,
    symbols: Vec<NativeSymbol>,
    positions: Vec<usize>,
}

impl NativePrefix {
    pub fn encode(
        base: Arc<PreparedBase>,
        ids: &[u32],
        positions: &[usize],
        expected_length: usize,
        limits: NativeLimits,
    ) -> Result<Self, NativeError> {
        validate_shape(ids.len(), positions, expected_length)?;
        let mut symbols = Vec::new();
        limits.reserve(Phase::Encode, Resource::Roots, &mut symbols, ids.len())?;
        for &id in ids {
            symbols.push(NativeSymbol::registered(&base, id)?);
        }
        Self::from_owned_symbols(base, symbols, positions, limits)
    }

    pub fn from_symbols(
        base: Arc<PreparedBase>,
        symbols: &[NativeSymbol],
        positions: &[usize],
        expected_length: usize,
        limits: NativeLimits,
    ) -> Result<Self, NativeError> {
        validate_shape(symbols.len(), positions, expected_length)?;
        for symbol in symbols {
            validate_symbol(&base, symbol)?;
        }
        let mut owned = Vec::new();
        limits.reserve(Phase::Encode, Resource::Roots, &mut owned, symbols.len())?;
        owned.extend_from_slice(symbols);
        Self::from_owned_symbols(base, owned, positions, limits)
    }

    fn from_owned_symbols(
        base: Arc<PreparedBase>,
        symbols: Vec<NativeSymbol>,
        positions: &[usize],
        limits: NativeLimits,
    ) -> Result<Self, NativeError> {
        let mut owned_positions = Vec::new();
        limits.reserve(
            Phase::Encode,
            Resource::Roots,
            &mut owned_positions,
            positions.len(),
        )?;
        owned_positions.extend_from_slice(positions);
        Ok(Self {
            base,
            symbols,
            positions: owned_positions,
        })
    }
    pub fn base(&self) -> &Arc<PreparedBase> {
        &self.base
    }
    pub fn symbols(&self) -> &[NativeSymbol] {
        &self.symbols
    }
    pub fn positions(&self) -> &[usize] {
        &self.positions
    }
    pub fn token_ids(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.symbols.iter().map(|s| s.id)
    }
    pub fn storage_bytes(&self) -> (usize, usize) {
        (
            size_of_val(self.symbols.as_slice()) + size_of_val(self.positions.as_slice()),
            self.symbols.capacity() * size_of::<NativeSymbol>()
                + self.positions.capacity() * size_of::<usize>(),
        )
    }

    /// La composición debe repetir la comprobación dentro de su intervalo medido.
    pub fn validate_for(
        &self,
        base: &Arc<PreparedBase>,
        expected_length: usize,
    ) -> Result<(), NativeError> {
        if !Arc::ptr_eq(&self.base, base) {
            return Err(error(NativeErrorKind::ForeignReference));
        }
        validate_shape(self.symbols.len(), &self.positions, expected_length)?;
        for symbol in &self.symbols {
            validate_symbol(base, symbol)?;
        }
        Ok(())
    }
}

fn error(kind: NativeErrorKind) -> NativeError {
    NativeError {
        phase: Phase::Encode,
        kind,
    }
}
fn validate_shape(length: usize, positions: &[usize], expected: usize) -> Result<(), NativeError> {
    if length == 0 || length != expected {
        return Err(error(NativeErrorKind::Shape {
            expected,
            actual: length,
        }));
    }
    if positions.len() != length {
        return Err(error(NativeErrorKind::Shape {
            expected: length,
            actual: positions.len(),
        }));
    }
    for (index, &actual) in positions.iter().enumerate() {
        if actual != index {
            return Err(error(NativeErrorKind::Position { index, actual }));
        }
    }
    Ok(())
}
fn validate_symbol(base: &PreparedBase, symbol: &NativeSymbol) -> Result<(), NativeError> {
    base.validate(&symbol.root).map_err(|mut e| {
        e.phase = Phase::Encode;
        e
    })?;
    if NativeSymbol::registered(base, symbol.id)?.root != symbol.root {
        return Err(error(NativeErrorKind::InvalidSymbol { id: symbol.id }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::TemplateBuilder;

    #[test]
    fn encoding_original_prefix_preserves_bos_order_positions_and_preparation() {
        let base = Arc::new(PreparedBase::vocabulary_fixture(512, 2));
        let before = base.storage();
        let builder = TemplateBuilder::new(base.clone(), 16, NativeLimits::default()).unwrap();
        let root = builder.port(0).unwrap();
        let template = builder.finish(&[root]).unwrap();
        let stats = template.stats();
        let ids = [1, 403, 407, 261, 378, 432, 383, 286];
        let prefix = NativePrefix::encode(
            base.clone(),
            &ids,
            &[0, 1, 2, 3, 4, 5, 6, 7],
            8,
            NativeLimits::default(),
        )
        .unwrap();
        assert!(prefix.token_ids().eq(ids));
        assert_eq!(prefix.positions(), &[0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(
            prefix.symbols()[0].root(),
            &base.vocabulary_symbol(1).unwrap()
        );
        assert_eq!(base.storage(), before);
        assert_eq!(template.stats(), stats);
        prefix.validate_for(&base, 8).unwrap();
        let (payload, capacity) = prefix.storage_bytes();
        assert!(payload > 0 && payload <= capacity);
    }

    #[test]
    fn rejects_range_length_positions_foreign_and_unregistered_symbols() {
        let base = Arc::new(PreparedBase::vocabulary_fixture(512, 1));
        let other = Arc::new(PreparedBase::vocabulary_fixture(512, 1));
        let limits = NativeLimits::default();
        assert!(matches!(
            NativePrefix::encode(base.clone(), &[512], &[0], 1, limits)
                .unwrap_err()
                .kind,
            NativeErrorKind::TokenRange { id: 512 }
        ));
        assert!(NativePrefix::encode(base.clone(), &[1], &[0], 8, limits).is_err());
        assert!(NativePrefix::encode(base.clone(), &[1, 2], &[0, 2], 2, limits).is_err());
        let foreign = NativeSymbol::registered(&other, 1).unwrap();
        assert_eq!(
            NativePrefix::from_symbols(base.clone(), &[foreign], &[0], 1, limits)
                .unwrap_err()
                .kind,
            NativeErrorKind::ForeignReference
        );
        let invalid = NativeSymbol {
            id: 1,
            root: base.vocabulary_symbol(2).unwrap(),
        };
        assert!(matches!(
            NativePrefix::from_symbols(base.clone(), &[invalid], &[0], 1, limits)
                .unwrap_err()
                .kind,
            NativeErrorKind::InvalidSymbol { id: 1 }
        ));
        let symbol = NativeSymbol::registered(&base, 1).unwrap();
        let prefix = NativePrefix::from_symbols(base.clone(), &[symbol], &[0], 1, limits).unwrap();
        assert!(prefix.validate_for(&other, 1).is_err());
        assert!(
            NativePrefix::encode(base, &[1], &[0], 1, NativeLimits { roots: 0, ..limits }).is_err()
        );
    }
}
