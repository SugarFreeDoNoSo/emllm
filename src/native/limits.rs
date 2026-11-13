use std::{
    collections::{HashMap, TryReserveError},
    error::Error,
    fmt,
    hash::Hash,
    mem::size_of,
};

/// Fases con fronteras independientes; evaluar nunca forma parte de componer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Load,
    Base,
    Template,
    Encode,
    Compose,
    Compile,
    Plan,
    EvaluateOriginal,
    EvaluatePlanned,
    Compare,
    Select,
    Emit,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Base => "base",
            Self::Template => "template",
            Self::Encode => "encode",
            Self::Compose => "compose",
            Self::Compile => "compile",
            Self::Plan => "plan",
            Self::EvaluateOriginal => "evaluate-original",
            Self::EvaluatePlanned => "evaluate-planned",
            Self::Compare => "compare",
            Self::Select => "select",
            Self::Emit => "emit",
        }
    }
}

/// Unidades lógicas, no estimaciones del RSS. Los límites son inclusivos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    PackageBytes,
    Tensors,
    BaseNodes,
    TemplateInstructions,
    Ports,
    Roots,
    ClosedNodes,
    ReachableNodes,
    NumericSlots,
}

impl Resource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PackageBytes => "package_bytes",
            Self::Tensors => "tensors",
            Self::BaseNodes => "base_nodes",
            Self::TemplateInstructions => "template_instructions",
            Self::Ports => "ports",
            Self::Roots => "roots",
            Self::ClosedNodes => "closed_nodes",
            Self::ReachableNodes => "reachable_nodes",
            Self::NumericSlots => "numeric_slots",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeErrorKind {
    ForeignReference,
    InvalidReference {
        index: usize,
    },
    EmptyConstants,
    Compile(crate::program::CompileError),
    Limit {
        resource: Resource,
        requested: usize,
        limit: usize,
    },
    SizeOverflow {
        resource: Resource,
    },
    /// `std` no distingue de forma estable el detalle de TryReserveError.
    Allocation {
        resource: Resource,
        detail: String,
    },
}

/// Un fallo conserva fase y categoría; no contiene una salida parcial válida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeError {
    pub phase: Phase,
    pub kind: NativeErrorKind,
}

impl NativeError {
    fn overflow(phase: Phase, resource: Resource) -> Self {
        Self {
            phase,
            kind: NativeErrorKind::SizeOverflow { resource },
        }
    }

    pub(crate) fn allocation(phase: Phase, resource: Resource, error: TryReserveError) -> Self {
        Self {
            phase,
            kind: NativeErrorKind::Allocation {
                resource,
                detail: error.to_string(),
            },
        }
    }
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.phase.as_str())?;
        match &self.kind {
            NativeErrorKind::ForeignReference => {
                write!(f, "reference belongs to another native context")
            }
            NativeErrorKind::InvalidReference { index } => {
                write!(f, "invalid native reference {index}")
            }
            NativeErrorKind::EmptyConstants => write!(f, "at least one constant is required"),
            NativeErrorKind::Compile(error) => write!(f, "{error}"),
            NativeErrorKind::Limit {
                resource,
                requested,
                limit,
            } => {
                write!(
                    f,
                    "{} budget {limit} exceeded by request {requested}",
                    resource.as_str()
                )
            }
            NativeErrorKind::SizeOverflow { resource } => {
                write!(f, "{} size overflow", resource.as_str())
            }
            NativeErrorKind::Allocation { resource, detail } => {
                write!(f, "{} allocation failed: {detail}", resource.as_str())
            }
        }
    }
}

impl Error for NativeError {}

/// Perfil estructural configurable, independiente del experimento anterior.
/// Cero es un presupuesto válido que rechaza el primer elemento. `Default`
/// corresponde a `tools/native-profile.json`; no reserva memoria anticipadamente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeLimits {
    pub package_bytes: usize,
    pub tensors: usize,
    pub base_nodes: usize,
    pub template_instructions: usize,
    pub ports: usize,
    pub roots: usize,
    pub closed_nodes: usize,
    pub reachable_nodes: usize,
    pub numeric_slots: usize,
}

impl Default for NativeLimits {
    fn default() -> Self {
        Self {
            package_bytes: 2_097_152,
            tensors: 64,
            base_nodes: 50_000_000,
            template_instructions: 250_000_000,
            ports: 1024,
            roots: 1_000_000,
            closed_nodes: 300_000_000,
            reachable_nodes: 300_000_000,
            numeric_slots: 300_000_000,
        }
    }
}

impl NativeLimits {
    pub fn limit(&self, resource: Resource) -> usize {
        match resource {
            Resource::PackageBytes => self.package_bytes,
            Resource::Tensors => self.tensors,
            Resource::BaseNodes => self.base_nodes,
            Resource::TemplateInstructions => self.template_instructions,
            Resource::Ports => self.ports,
            Resource::Roots => self.roots,
            Resource::ClosedNodes => self.closed_nodes,
            Resource::ReachableNodes => self.reachable_nodes,
            Resource::NumericSlots => self.numeric_slots,
        }
    }

    pub fn check(
        &self,
        phase: Phase,
        resource: Resource,
        requested: usize,
    ) -> Result<(), NativeError> {
        let limit = self.limit(resource);
        if requested > limit {
            return Err(NativeError {
                phase,
                kind: NativeErrorKind::Limit {
                    resource,
                    requested,
                    limit,
                },
            });
        }
        Ok(())
    }

    /// Suma comprobada antes de reservar o mutar contadores. Para el grafo cerrado
    /// `existing` incluye toda la base residente, también nodos no alcanzables.
    pub fn checked_add(
        &self,
        phase: Phase,
        resource: Resource,
        existing: usize,
        additional: usize,
    ) -> Result<usize, NativeError> {
        let total = existing
            .checked_add(additional)
            .ok_or_else(|| NativeError::overflow(phase, resource))?;
        self.check(phase, resource, total)?;
        Ok(total)
    }

    /// Producto comprobado de dimensiones; una forma escalar vacía tiene un elemento.
    pub fn checked_elements(
        &self,
        phase: Phase,
        resource: Resource,
        dimensions: &[usize],
    ) -> Result<usize, NativeError> {
        let count = dimensions.iter().try_fold(1usize, |n, &dimension| {
            n.checked_mul(dimension)
                .ok_or_else(|| NativeError::overflow(phase, resource))
        })?;
        self.check(phase, resource, count)?;
        Ok(count)
    }

    /// Bytes del payload solicitado; el asignador y las capacidades pueden costar más.
    pub fn checked_bytes<T>(
        &self,
        phase: Phase,
        resource: Resource,
        elements: usize,
    ) -> Result<usize, NativeError> {
        let bytes = elements
            .checked_mul(size_of::<T>())
            .filter(|&n| n <= isize::MAX as usize)
            .ok_or_else(|| NativeError::overflow(phase, resource))?;
        self.check(phase, resource, elements)?;
        Ok(bytes)
    }

    /// Reserva recuperable sin cambiar longitud/contenido ante errores. El límite
    /// cuenta elementos lógicos, no la capacidad especulativa del vector.
    pub fn reserve<T>(
        &self,
        phase: Phase,
        resource: Resource,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), NativeError> {
        let count = self.checked_add(phase, resource, values.len(), additional)?;
        self.checked_bytes::<T>(phase, resource, count)?;
        values
            .try_reserve(additional)
            .map_err(|error| NativeError::allocation(phase, resource, error))
    }

    pub(crate) fn reserve_map<K: Eq + Hash, V>(
        &self,
        phase: Phase,
        resource: Resource,
        values: &mut HashMap<K, V>,
        additional: usize,
    ) -> Result<(), NativeError> {
        let count = self.checked_add(phase, resource, values.len(), additional)?;
        self.checked_bytes::<(K, V)>(phase, resource, count)?;
        values
            .try_reserve(additional)
            .map_err(|error| NativeError::allocation(phase, resource, error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservation_errors_keep_the_phase_and_std_diagnostic() {
        // Fallo determinista de std sin intentar agotar la RAM del host.
        let error = Vec::<u64>::new().try_reserve(usize::MAX).unwrap_err();
        let error = NativeError::allocation(Phase::Base, Resource::BaseNodes, error);
        assert_eq!(error.phase, Phase::Base);
        assert!(matches!(
            error.kind,
            NativeErrorKind::Allocation {
                resource: Resource::BaseNodes,
                ..
            }
        ));
        assert!(
            error
                .to_string()
                .starts_with("base: base_nodes allocation failed:")
        );
    }
}
