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
//! assert_eq!(arena.node_count(), 2);
//! assert_eq!(arena.node(e.index()), Some(Node::Magma(one.index(), one.index())));
//! arena.validate_roots(&[e])?;
//! # Ok::<(), emllm::expr::BuildError>(())
//! ```
//!
//! Los índices observados son locales: no permiten fabricar referencias válidas
//! para otra arena. La inspección devuelve nodos por copia e iteradores de
//! longitud exacta; cambiar una copia no modifica la estructura.
//!
//! # Compilación y propiedad
//!
//! [`program::compile`] comprueba una lista no vacía de raíces de la misma arena,
//! sus dependencias y un presupuesto de nodos alcanzables. Produce un
//! [`program::Program`] propietario: hijos antes que padres, raíces en el orden
//! solicitado y repeticiones conservadas. El recorrido es iterativo y no evalúa
//! operaciones numéricas. Solo la clausura alcanzable pasa al snapshot.
//!
//! La arena puede ampliarse o liberarse después. Sus [`expr::NodeId`] no son
//! índices del snapshot: [`program::Program::node`] resuelve las posiciones de
//! inspección y diagnóstico del programa correspondiente. Ninguna vista permite
//! cambiar nodos o raíces. Los ejemplos de [`program::Program`] verifican tanto
//! su uso válido como el rechazo de intentos externos de invalidarlo.
//!
//! # Responsabilidades y dependencias
//!
//! `expr` define el lenguaje cerrado y protege la arena; `synth` compone ese
//! lenguaje; `program` valida y posee la vista ejecutable; `eval` consume esa
//! vista; `network` compone síntesis y expresiones. Los módulos de construcción
//! no ejecutan el evaluador. La referencia convencional solo existe en pruebas
//! y demostración. No hay dependencias de ejecución externas a la biblioteca
//! estándar, configuración global mutable ni E/S en estas etapas del núcleo.
//!
//! # Recursos y garantías
//!
//! El presupuesto de construcción cuenta nodos únicos de toda la arena.
//! Reutilizar un nodo internado no lo consume otra vez. Un error de construcción
//! no entrega una nueva raíz como éxito; los nodos anteriores siguen siendo
//! válidos. El número de nodos no es una medida exacta de bytes o de tiempo.
//!
//! La estructura válida no garantiza que una evaluación numérica tenga éxito.
//! El dominio del logaritmo y la finitud de los intermedios se comprueban al
//! evaluar; la identidad algebraica y la precisión finita son propiedades distintas.

#![forbid(unsafe_code)]

pub mod eval;
pub mod execution;
pub mod expr;
pub mod model_io;
pub mod network;
pub mod program;
pub mod synth;
