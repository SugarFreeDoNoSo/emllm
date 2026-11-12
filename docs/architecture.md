# Contratos del núcleo Rust

El crate `emllm` representa un lenguaje cerrado: `Node::One` y
`Node::Magma(left, right)`. Sobre los reales, sus significados son 1 y
`exp(left) - ln(right)`, con segundo argumento positivo. Las expansiones
algebraicas y sus condiciones están en [síntesis](synthesis.md).

## Ciclo de vida y propiedad

```text
Rational / Network221 → Arena + NodeId → compile → Program
                                           │        ├→ inspección
                                           │        └→ evaluate + presupuesto
                                           └→ CompileError
```

- `Arena::new(max_nodes)` fija el límite de nodos únicos construidos. La arena
  es append-only, no reutiliza posiciones y no implementa `Clone`.
- `NodeId` encapsula una posición y un token `Arc` privado. Los handles mantienen
  vivo ese token aunque se libere la arena. Dos posiciones iguales de arenas
  distintas no son la misma referencia. `magma` valida ambos operandos antes de
  internar; el internado conserva el orden y compara estructura, nunca valores.
- `Arena::validate_roots` comprueba presencia y procedencia. No produce un
  programa ni certifica validez numérica.
- `compile(&arena, &roots, max_nodes)` exige raíces no vacías de esa arena,
  valida sus dependencias y copia solo la clausura alcanzable. El recorrido es
  iterativo, visita primero el operando izquierdo y emite hijos antes que padres.
  Mantiene el orden y las repeticiones de las raíces. La misma estructura y lista
  de raíces dan el mismo orden; las tablas hash no se recorren para decidirlo.
- `Program` tiene campos privados y posee el snapshot. Se puede ampliar o liberar
  la arena sin alterarlo. `Clone` copia un programa ya válido, con coste lineal;
  distintos snapshots pueden duplicar almacenamiento.
- `Arena::nodes()` y `Program::nodes()` devuelven iteradores de longitud exacta
  con valores `Node`; `node(index)` devuelve `Option<Node>` por copia y
  `node_count()` cuenta nodos lógicos. Estas consultas no asignan una colección
  de nodos y cambiar una copia no modifica la estructura. `Program::roots()`
  conserva su slice inmutable. Los índices de operandos,
  raíces y errores son locales al programa que los publicó: no son `NodeId`,
   identificadores persistentes ni referencias intercambiables entre snapshots.
- `Program::stats()` consulta en O(1) los conteos V/E/R/D calculados al compilar.
  No evalúa operaciones numéricas; sus definiciones, costes y alcance están en
  [estadísticas y complejidad](complexity.md).
- `evaluate(&program, max_nodes)` devuelve una salida por posición de raíz o un
  error. No acepta una arena, un handle ni una lista de nodos sin validar. No
  vuelve a validar procedencia/ciclos ni introduce `unsafe` para evitar los
  accesos seguros de Rust.

```rust
use emllm::{eval::evaluate, expr::Arena, program::compile};

let mut arena = Arena::new(3);
let one = arena.one().unwrap();
let e = arena.magma(&one, &one).unwrap();
let program = compile(&arena, &[e.clone(), one.clone(), e.clone()], 2).unwrap();
arena.magma(&e, &one).unwrap();
drop(arena);
assert_eq!(program.roots(), &[1, 0, 1]);
assert!(program.node(program.roots()[0]).is_some());
let outputs = evaluate(&program, 2).unwrap();
assert!((outputs[0] - std::f64::consts::E).abs() <= 1e-12);
assert_eq!(outputs[1], 1.0);
assert_eq!(outputs[0], outputs[2]); // Mismo nodo, misma llamada.
```

Los doctests de `NodeId`, `Program` y `evaluate` incluyen controles positivos y
`compile_fail`: campos privados, vistas inmutables y tipos de entrada. Ejecutar
`cargo test --doc -- --show-output` permite revisar sus diagnósticos. En Rust
estable, `compile_fail` solo exige un fallo de compilación, no un código de error
concreto. Los controles positivos y la revisión evitan aceptar fallos debidos a
imports o preparación incorrectos.

## Errores y presupuestos por etapa

Los consumidores pueden hacer `match` sobre los tipos/variantes, sin analizar
mensajes de `Display`. Los errores envueltos preservan `Error::source`.

| Etapa / tipo | Categorías | Identificación |
| --- | --- | --- |
| Frontera racional / `RationalError` | `ZeroDenominator`, `OutsideDomain` normalizado | Metadatos racionales, sin nodo |
| Construcción / `BuildError` | `ForeignReference`, `InvalidReference`, `EmptyRoots`, `NodeLimit` | Posición de arena solo para referencia inválida |
| Compilación / `CompileError` | `Roots(BuildError)`, `InvalidReference`, `Cycle`, `NodeLimit` | Índice de origen si se detecta referencia/ciclo; aún no existe snapshot |
| Red / `NetworkError` | `InputDimension`, `Build(BuildError)` | Dimensiones o error de construcción envuelto |
| Evaluación / `EvalError` + `EvalErrorKind` | `LogDomain`, `ExponentialOverflow`, `NonFinite`, `NodeLimit` | `Some(index)` del programa para fallo numérico; `None` al rechazar presupuesto antes de ejecutar |

El límite de compilación cuenta nodos únicos alcanzables; el de evaluación,
nodos del snapshot. Las posiciones de raíz, incluidos duplicados, consumen
almacenamiento adicional. Ninguno de estos límites es una cuota de bytes o
tiempo, ni captura fallos del asignador de memoria.

Un fallo de síntesis puede dejar nodos internos válidos en la arena, pero no
entrega la raíz fallida como éxito. No hay rollback transaccional. Compilar no
entrega programas parciales; evaluar no devuelve vectores parciales exitosos,
aunque alguna raíz ya tuviera valor. Los programas previos permanecen intactos.

## Responsabilidades y composición

| Módulo | Decisión que encapsula | Dependencias internas de producción |
| --- | --- | --- |
| `expr` | Lenguaje, procedencia, internado y presupuesto de arena | Ninguna |
| `synth` | Normalización racional y expansiones constructivas | `expr` |
| `program` | Validación, orden topológico y snapshot inspeccionable | `expr` |
| `eval` | Semántica numérica, temporales y errores | Vista `Node`/`Program` de `program` |
| `network` | Forma 2-2-1 y composición de capas | `synth`, `expr` |
| Ejemplo | Coordinar etapas y presentar resultados | API pública y referencia independiente |

La revisión de imports y llamadas confirma que síntesis y compilación no llaman
al evaluador. Las únicas llamadas de producción a `exp`/`ln` están en `eval`.
La referencia de `examples/support/reference.rs` solo se usa en pruebas y ejemplo.
La biblioteca utiliza la biblioteca estándar y no ejecuta E/S ni lee configuración
ambiental. Los buffers numéricos pertenecen a cada llamada.

### SOLID como criterios de diseño

- **SRP:** separar representación, síntesis, validación, semántica numérica y red
  por sus invariantes y motivos de cambio. Varias comprobaciones de dominio
  pueden pertenecer a una misma responsabilidad; contar `if` no decide cohesión.
- **OCP:** añadir constructores componiendo One/Magma sin ampliar el lenguaje
  ejecutable. El `enum` cerrado y el `match` exhaustivo protegen deliberadamente
  esa frontera; no se necesita un objeto extensible por instrucción.
- **LSP:** una futura implementación sustituible tendrá que respetar dominio,
  errores, resultados, efectos y tolerancias. Compartir una firma o implementar
  el mismo `trait` no demuestra sustitución correcta.
- **ISP:** construir requiere la arena mutable; evaluar solo requiere el programa
  inmutable y su presupuesto. Estas capacidades pequeñas evitan interfaces grandes
  sin crear un `trait` por función.
- **DIP:** la red compone contratos del lenguaje y de síntesis, sin depender de
  los detalles de evaluación `f64`. Una nueva abstracción se justificará por una
  variación real y por el contrato que necesita su consumidor.

Se usan funciones concretas para composición y métodos para proteger invariantes.
No se introduce un backend genérico: una variación pequeña podría recibir una
función/cierre; un conjunto coherente de operaciones podría justificar un `trait`.
Primero se especificarían y probarían las obligaciones; después se elegiría
despacho estático o dinámico. Extraer funciones solo para reducir una métrica no
prueba ninguna de estas propiedades.

## Estructura válida y evaluación finita

`Program` certifica estructura, no positividad, finitud ni precisión. Con
`a=M(1,1)`, `b=M(a,1)`, `c=M(b,1)` y `d=M(1,c)`, se tiene `d<0` sobre los reales:
`M(1,d)` compila y falla por dominio; `M(c,1)` compila y desborda en `f64`.
Ambos siguen siendo inspeccionables después del fallo.

El evaluador comprueba finitud de operandos, positividad del segundo argumento,
finitud de la exponencial y del resultado. No usa clamps ni reemplaza una
expansión por aritmética convencional. Conserva subnormales y cero por
subdesbordamiento; una operación posterior puede fallar por dominio. La
cancelación puede degradar un resultado real pequeño.

La precisión de [`f64::exp`](https://doc.rust-lang.org/std/primitive.f64.html#method.exp)
y [`f64::ln`](https://doc.rust-lang.org/std/primitive.f64.html#method.ln) no está
especificada y puede variar entre plataformas, versiones e incluso llamadas de
la misma ejecución. Composición funcional y aislamiento no implican transparencia
referencial numérica estricta ni igualdad bit a bit entre llamadas.
Las pruebas usan `1e-12` para e, `1e-9` para racionales, `1e-8` para las operaciones
documentadas y `1e-6` para las tres fixtures neuronales. Son criterios locales de
aceptación; no constituyen una cota global para cualquier DAG.

## Evidencia de los contratos

| Contrato | Verificación del crate |
| --- | --- |
| Procedencia y referencias opacas | `tests/contracts.rs`, pruebas y doctests de `src/expr.rs` |
| Raíces, compartición, referencias inexistentes, ciclos y recorrido profundo | `tests/contracts.rs` y pruebas internas de `src/program.rs` |
| Snapshot independiente e inspección inmutable | `tests/contracts.rs`, `tests/statistics.rs` y doctests de `Program` |
| Entrada validada, errores numéricos, salidas completas y aislamiento | Doctests de `evaluate`, `tests/evaluation.rs`, `tests/contracts.rs` y pruebas internas de `src/eval.rs` |
| Estadísticas exactas y estables, sin evaluación | `tests/statistics.rs` y pruebas internas de `src/program.rs` |
| Tolerancias de la base | `tests/synthesis.rs`, `tests/network.rs` y `examples/infer.rs` |

La [guía de complejidad](complexity.md) registra además cobertura de fuentes,
repetibilidad, estados no disponibles/incompletos y funcionamiento sin el
analizador. Esas comprobaciones documentales complementan las pruebas del crate.
