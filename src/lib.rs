//! Construcción simbólica y evaluación de programas cerrados One/Magma.
//!
//! # Lenguaje y etapas
//!
//! En aritmética real, `One = 1` y `Magma(a, b) = exp(a) - ln(b)`,
//! con el segundo argumento estrictamente positivo. Los nombres de operaciones
//! aritméticas de la interfaz de síntesis son constructores, no instrucciones
//! adicionales. Entradas, pesos y sesgos deben expandirse antes de evaluar.
//!
//! La arena solo almacena estructura. Interna nodos por forma y orden de hijos;
//! no ejecuta `exp`/`ln` ni decide igualdad comparando valores numéricos.
//! Las referencias son propias de una arena y solo pueden apuntar a nodos ya
//! construidos, de modo que las operaciones públicas no pueden crear ciclos.
//!
//! # Construcción e inspección sin evaluación
//!
//! ```
//! use emllm::expr::{Arena, Node};
//!
//! let mut arena = Arena::new(32);
//! let one = arena.one()?;
//! let e = arena.magma(&one, &one)?;
//! let repeated = arena.magma(&one, &one)?;
//! assert_eq!(e, repeated);
//! assert_eq!(arena.nodes().len(), 2);
//! assert_eq!(arena.nodes()[e.index()], Node::Magma(one.index(), one.index()));
//! arena.validate_roots(&[e])?;
//! # Ok::<(), emllm::expr::BuildError>(())
//! ```
//!
//! Los índices observados son locales: no permiten fabricar referencias válidas
//! para otra arena. La inspección devuelve una vista de solo lectura.
//!
//! # Recursos y garantías
//!
//! El presupuesto de construcción cuenta nodos únicos de toda la arena.
//! Reutilizar un nodo internado no lo consume otra vez. Un error de construcción
//! no entrega una nueva raíz como éxito; los nodos anteriores siguen siendo
//! válidos. El número de nodos no es una medida exacta de bytes o de tiempo.
//!
//! La estructura válida no garantiza que una evaluación numérica tenga éxito.
//! El dominio del logaritmo y la finitud de los intermedios se comprobarán al
//! evaluar; la identidad algebraica y la precisión finita son propiedades distintas.

#![forbid(unsafe_code)]

pub mod eval;
pub mod expr;
pub mod network;
pub mod synth;
