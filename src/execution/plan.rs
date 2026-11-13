use std::{error::Error, fmt, mem::size_of};

use crate::program::Program;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanLimits {
    /// Incluye One, independientemente del número de posiciones reutilizables.
    pub max_nodes: usize,
    /// Incluye la posición 0, reservada permanentemente para One.
    pub max_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    NodeLimit { limit: usize },
    SlotLimit { limit: usize },
    Allocation { storage: &'static str },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "planning {self:?}")
    }
}

impl Error for PlanError {}

#[derive(Clone, Copy, Debug)]
pub(super) struct Instruction {
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) destination: usize,
}

/// Plan validado que presta exactamente el programa de sus diagnósticos.
/// Una instrucción por par, en el mismo orden; la instrucción i ejecuta i+1.
/// No es posible construirlo desde instrucciones arbitrarias.
///
/// ```compile_fail
/// use emllm::{execution::ExecutionPlan, program::Program};
/// fn fabricate(program: &Program) {
///     let plan = ExecutionPlan { program, instructions: vec![], root_slots: vec![0], slots: 1 };
/// }
/// ```
///
/// ```compile_fail
/// use emllm::{execution::{plan, PlanLimits}, expr::Arena, program::compile};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let program = compile(&arena, &[one], 1).unwrap();
/// let mut plan = plan(&program, PlanLimits { max_nodes: 1, max_slots: 1 }).unwrap();
/// plan.root_slots()[0] = 99;
/// ```
///
/// ```compile_fail
/// use emllm::{execution::{plan, PlanLimits}, expr::Arena, program::compile};
/// let mut arena = Arena::new(1);
/// let one = arena.one().unwrap();
/// let program = compile(&arena, &[one], 1).unwrap();
/// let plan = plan(&program, PlanLimits { max_nodes: 1, max_slots: 1 }).unwrap();
/// drop(program);
/// println!("{:?}", plan.stats());
/// ```
#[derive(Debug)]
pub struct ExecutionPlan<'program> {
    pub(super) program: &'program Program,
    pub(super) instructions: Vec<Instruction>,
    pub(super) root_slots: Vec<usize>,
    pub(super) slots: usize,
}

/// Payloads del plan, no RSS. El programa prestado se contabiliza por separado.
/// Las capacidades incluyen elementos ocupados y espacio sobrante del vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanStats {
    pub instructions: usize,
    pub slots: usize,
    pub roots: usize,
    pub instruction_bytes: usize,
    pub instruction_payload_bytes: usize,
    pub instruction_capacity_bytes: usize,
    pub root_payload_bytes: usize,
    pub root_capacity_bytes: usize,
    /// Buffer numérico requerido al ejecutar; no está residente en el plan.
    pub value_bytes: usize,
    /// Estructura inline, incluidas las cabeceras de los dos vectores.
    pub metadata_bytes: usize,
}

impl ExecutionPlan<'_> {
    pub fn program(&self) -> &Program {
        self.program
    }

    /// Copias [izquierda, derecha, destino] en índices del buffer numérico.
    pub fn instructions(&self) -> impl ExactSizeIterator<Item = [usize; 3]> + '_ {
        self.instructions
            .iter()
            .map(|i| [i.left, i.right, i.destination])
    }

    /// Posiciones de resultados, conservando el orden y duplicados de las raíces.
    pub fn root_slots(&self) -> &[usize] {
        &self.root_slots
    }

    /// Consulta O(1), sin ejecutar nodos ni asignar buffers.
    pub fn stats(&self) -> PlanStats {
        PlanStats {
            instructions: self.instructions.len(),
            slots: self.slots,
            roots: self.root_slots.len(),
            instruction_bytes: size_of::<Instruction>(),
            instruction_payload_bytes: self.instructions.len() * size_of::<Instruction>(),
            instruction_capacity_bytes: self.instructions.capacity() * size_of::<Instruction>(),
            root_payload_bytes: self.root_slots.len() * size_of::<usize>(),
            root_capacity_bytes: self.root_slots.capacity() * size_of::<usize>(),
            value_bytes: self.slots * size_of::<f64>(),
            metadata_bytes: size_of::<Self>(),
        }
    }
}

fn reserve<T>(count: usize, storage: &'static str) -> Result<Vec<T>, PlanError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| PlanError::Allocation { storage })?;
    Ok(values)
}

/// Planifica en O(V+E+R), sin evaluación numérica ni cambio del orden topológico.
/// Los límites se comprueban antes de entregar un plan; un fallo no inventa un
/// nodo lógico numéricamente fallido. Los temporales de planificación se descartan.
pub fn plan(program: &Program, limits: PlanLimits) -> Result<ExecutionPlan<'_>, PlanError> {
    let nodes = program.node_count();
    if nodes > limits.max_nodes {
        return Err(PlanError::NodeLimit {
            limit: limits.max_nodes,
        });
    }
    if limits.max_slots == 0 {
        return Err(PlanError::SlotLimit { limit: 0 });
    }
    let mut last_use = reserve(nodes, "last_use")?;
    last_use.resize(nodes, 0);
    let mut locations = reserve(nodes, "locations")?;
    locations.resize(nodes, 0);
    let mut free = reserve(nodes, "free_slots")?;
    let mut instructions = reserve(program.pairs().len(), "instructions")?;
    let mut root_slots = reserve(program.roots().len(), "roots")?;
    for (row, &[left, right]) in program.pairs().iter().enumerate() {
        last_use[left] = row + 1;
        last_use[right] = row + 1;
    }
    // El índice nodes queda fuera de todas las instrucciones: raíces vivas al final.
    for &root in program.roots() {
        last_use[root] = nodes;
    }
    let mut slots = 1;
    for (row, &[left, right]) in program.pairs().iter().enumerate() {
        let node = row + 1;
        let (left_slot, right_slot) = (locations[left], locations[right]);
        if left != 0 && last_use[left] == node {
            free.push(left_slot);
        }
        if right != 0 && right != left && last_use[right] == node {
            free.push(right_slot);
        }
        let destination = if let Some(slot) = free.pop() {
            slot
        } else {
            if slots == limits.max_slots {
                return Err(PlanError::SlotLimit {
                    limit: limits.max_slots,
                });
            }
            let slot = slots;
            slots += 1;
            slot
        };
        locations[node] = destination;
        instructions.push(Instruction {
            left: left_slot,
            right: right_slot,
            destination,
        });
    }
    root_slots.extend(program.roots().iter().map(|&root| locations[root]));
    Ok(ExecutionPlan {
        program,
        instructions,
        root_slots,
        slots,
    })
}
