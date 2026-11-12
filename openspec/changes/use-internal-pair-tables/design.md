# Design

## Context

Véase `proposal.md` para la motivación y el alcance confirmado. El usuario eligió una representación interna, no un formato de archivo, y aceptó cambiar la inspección de slices/referencias a iteradores y valores por copia.

Estado observado al planificar:

- `expr::Arena` guarda `Vec<Node>` y `HashMap<Node, usize>`; `Node` es `One | Magma(usize, usize)`. `NodeId` contiene índice y un token `Arc<()>` privado.
- `program::Program` posee `Vec<Node>`, raíces y estadísticas V/E/R/D. `compile_nodes` recorre iterativamente la clausura alcanzable y remapea índices.
- `eval::evaluate` recibe exclusivamente `&Program`, comprueba su presupuesto y conserva un `Vec<f64>` local a la llamada. Los `f64` no almacenan la estructura del DAG.
- `nodes()` devuelve `&[Node]` en arena y programa; `Program::node()` devuelve `Option<&Node>`. Pruebas y doctests utilizan indexing, `.iter()`, `.to_vec()` y comparaciones de slices.
- `tests/contracts.rs`, `tests/statistics.rs`, `tests/evaluation.rs`, `tests/synthesis.rs` y `tests/network.rs` cubren procedencia, propiedad, errores, estadísticas y fixtures numéricas. `docs/architecture.md` explica los contratos actuales.
- `implement-one-magma-eml` está completado; `formalize-rust-core-contracts` está en 15/23, con trabajo local en la demostración y el seguimiento. Este plan no completa ni altera ese trabajo. Antes de aplicar se requiere terminar y verificar la dependencia, incluida su guía de complejidad y procedimiento de análisis.

Es un cambio de modelo transversal a construcción, compilación, inspección y evaluación; por ello se incluye este diseño.

## Goals / Non-Goals

**Goals:**

- Hacer explícita la correspondencia entre índice lógico y fila física sin cambiar el alfabeto One/Magma.
- Mantener una única representación estructural compacta por arena o snapshot, con vistas lógicas calculadas a demanda.
- Permitir una migración por pasos verificables que mantenga el crate compilable y preserve los contratos existentes.
- Cuantificar los elementos estructurales ocupados, declarando las partes de memoria que ese modelo excluye.

**Non-Goals:**

- Diseñar un codec persistente, endianness, importación de tablas externas, enteros arbitrarios o binario inverso.
- Cambiar las identidades algebraicas, parámetros neuronales, precisión `f64`, tolerancias o política de errores.
- Introducir índices estrechos, un backend intercambiable, SIMD, memoización global o reutilización de temporales numéricos.
- Prometer un porcentaje de ahorro total o una mejora de tiempo sin mediciones específicas.

## Decisions

### 1. Tabla contigua de pares con identidad logica separada

Usar `Vec<[usize; 2]>`: cada fila contiene únicamente los índices lógicos de los operandos, en orden. El identificador 0 representa One; la fila `k` representa `Magma` con identificador `k+1`.

```text
Vista logica                 Tabla fisica
0: One                       (sin fila)
1: Magma(0,0)                fila 0: [0,0]
2: Magma(1,1)                fila 1: [1,1]

raices [2,0,2]               orden y duplicados conservados
```

El 0 es una dirección reservada, no el valor numérico cero. La expresión que sintetiza cero sigue siendo un DAG One/Magma. One no lleva un `f64` almacenado; el evaluador produce 1.0 cuando ejecuta esa forma.

Conservar `usize` mantiene el dominio de índices y los presupuestos de la base, evitando un nuevo límite de 32 bits y conversiones de estrechamiento. La tabla es una representación en memoria dependiente del target, no una especificación de bytes portables. El array de dos posiciones hace explícito el tamaño de la fila.

**Alternativas:** índices `u32` reducirían más el payload en targets de 64 bits pero introducirían límites y errores nuevos; empaquetado variable complicaría acceso y validación; codificar el árbol completo perdería compartición. Se posponen esas opciones.

### 2. One perezoso y almacenamiento privado compartido

Introducir un pequeño tipo privado al crate, `PairTable`, dentro de `expr`, reutilizable por `Arena` y `Program`. Contiene el vector de pares y una marca de presencia de One. Sus métodos internos centralizan el conteo lógico, la consulta por índice y el iterador, sin nuevas abstracciones públicas ni dependencias.

Invariantes de los valores válidos:

- Tabla no inicializada: One ausente, cero filas y cero nodos lógicos.
- Tabla inicializada: One presente y `node_count = pairs.len() + 1`.
- Nunca hay filas si One está ausente.
- Cada nueva fila de identificador `i` referencia hijos existentes menores que `i`.
- Un `Program` válido siempre contiene One, incluso si su tabla no tiene filas. La lista de raíces es no vacía y cada raíz resuelve en su snapshot.

La arena nueva continúa vacía. `one()` activa One solo tras comprobar presupuesto; repetirla no consume otra unidad. `magma()` valida la procedencia de ambos handles, busca el par en `HashMap<[usize; 2], usize>` y solo comprueba el límite para una inserción nueva. Conserva el token de identidad y no reutiliza índices.

Los mapas de internado son índices auxiliares sobre pares, no otra colección completa de nodos etiquetados. No se exige eliminar esa duplicación de claves. Se mantienen los efectos documentados de fallos de síntesis: pueden quedar nodos válidos anteriores, sin raíz fallida entregada como éxito.

`PairTable` no concede una vía pública para importar índices sin validar. Las fixtures internas pueden preparar representaciones malformadas con acceso limitado a pruebas; ningún constructor público nuevo fabrica `Program` ni `NodeId`.

**Alternativas:** considerar One siempre disponible modificaría el contrato de arena vacía y presupuesto cero; reservar una fila ficticia desperdiciaría almacenamiento y complicaría los conteos; replicar las reglas en arena y programa facilitaría errores de desplazamiento.

### 3. Inspeccion por valor y vista fisica de solo lectura

Conservar `Node` como enum público cerrado y `Copy`, reconstruido por consulta. Tanto `Arena` como `Program` ofrecerán:

| Operación | Contrato propuesto |
| --- | --- |
| `node_count() -> usize` | Número lógico de nodos, incluido One si existe |
| `pairs() -> &[[usize; 2]]` | Filas residentes, inmutables, sin copia |
| `node(index) -> Option<Node>` | One para 0 si existe; Magma de fila index-1; ausencia fuera de rango |
| `nodes() -> impl ExactSizeIterator<Item = Node> + '_` | Valores en orden lógico, sin asignar una colección |

`Program::roots()` conserva su slice inmutable y `stats()` conserva su consulta en tiempo constante. Los identificadores del snapshot siguen siendo distintos de los handles opacos de construcción, aunque ambos puedan mostrar el número 0.

Cambiar una copia `Node` no modifica la tabla. Las pruebas negativas comprobarán la imposibilidad de escribir en `pairs()` o `roots()` y la privacidad de los campos; un ejemplo que solo falle por la desaparición de un método no constituye evidencia suficiente de encapsulación.

Migración de consumidores:

- `.nodes().len()` puede usar la longitud exacta del iterador, pero presupuestos y reservas usarán `node_count()` explícitamente.
- `.nodes().iter()` pasa a `.nodes()`.
- Comparaciones/capturas de pruebas que necesitan una colección usan `.nodes().collect::<Vec<_>>()`; son asignaciones explícitas del consumidor, no una caché del núcleo.
- Indexing se sustituye por `node(index)` y `Some(&Node)` pasa a `Some(Node)`.
- La ausencia se consulta por `node_count() == 0` o `node(index)`, sin depender de extensiones inestables de iteradores.

**Alternativas:** conservar `&[Node]` exigiría materializar o mantener la colección antigua; una caché perezosa haría que inspeccionar aumentase permanentemente el almacenamiento. La adaptación de firmas fue elegida expresamente por el usuario.

### 4. Compilacion iterativa sin coleccion intermedia de nodos

Adaptar la lectura del compilador para obtener un `Node` por índice, sin convertir el iterador a `Vec<Node>`. Un lector privado por función/cierre permite mantener el recorrido y las fixtures internas mientras cambia el almacenamiento; no es una nueva interfaz pública de backends.

Conservar validación de raíces, estados de visita, orden de operandos y presupuesto por nodo lógico. El nodo lógico 0 se procesa y cobra una vez, aunque no emita una fila. Cada salida Magma añade un par remapeado y recibe identificador igual al número de fila más uno. Las raíces se remapean en el orden original y conservan duplicados.

Los vectores de profundidades y los mapas/stack de validación siguen siendo temporales lineales. Calcular V/E/R/D al compilar: `V=M+1`, `E=2M`, `R=roots.len()` y profundidad de One 0. El coste estructural sigue el modelo O(V+E+R) con operaciones de mapas de coste esperado; el presupuesto de nodos no limita por sí solo el número de posiciones de raíz.

Fixtures internas de la nueva codificación, con One presente:

- Una fila `[2,0]`, raíz 1: referencia inexistente 2.
- Una fila `[1,0]`, raíz 1: ciclo propio.
- Filas `[2,0]` y `[1,0]`, raíz 1: ciclo entre filas.

El compilador debe conservar categorías identificables y no producir snapshots parciales. Las posiciones de origen en errores de compilación son índices lógicos; las de errores de evaluación pertenecen al snapshot remapeado.

**Alternativas:** confiar únicamente en la topología garantizada por la arena eliminaría la verificación de fixtures malformadas exigida por los contratos; expandir a nodos etiquetados antes de validar reintroduciría una colección innecesaria.

### 5. Evaluacion directa de los pares validados

Comprobar `program.node_count()` contra el límite antes de asignar valores. Inicializar el temporal correspondiente a One con 1.0 y recorrer las filas en su orden. La fila `k` ejecuta exactamente la semántica Magma de sus dos valores hijos y usa `k+1` como identificador de diagnóstico. Recoger las raíces con sus índices lógicos sin alterar el orden.

Reutilizar la función numérica existente, sus comprobaciones y las categorías de error. No aplicar simplificaciones especiales por tratarse de pares iguales o por reconocer expansiones de síntesis. Los buffers de valores siguen siendo locales a cada llamada, y una evaluación fallida no modifica la tabla ni devuelve salidas parciales.

**Alternativas:** iterar las vistas `Node` también sería semánticamente válido, pero reconstruirlas para cada nodo no es necesario cuando el programa ya acredita la validez de todos sus pares. Sustituir el buffer `f64` por índices o bits mezclaría estructura con resultados numéricos y no resolvería la evaluación de exp/ln.

### 6. Evidencia de almacenamiento con alcance declarado

Definir `M=pairs.len()`, `V=node_count()` y `W=size_of::<usize>()`. El payload de los elementos de pares ocupados es `M*size_of::<[usize;2]>() = 2*M*W`. El modelo de elementos del formato anterior es `V*size_of::<Node>()` en el mismo target. El enum se conserva como vista, por lo que su tamaño puede consultarse sin guardar una colección del grafo.

El ejemplo informará por programa `pair_rows`, `pair_payload_bytes` y `tagged_node_payload_model_bytes`, junto a V/E/R/D, presupuestos y error numérico. La guía registrará revisión, comando, Rust/target y tamaños de tipos. No se asignará un tamaño fijo al enum de Rust antes de medirlo en ese entorno.

Estas cifras excluyen capacidad no usada de los vectores, cabeceras, marca de One, mapas de internado, tokens de arena, raíces, estadísticas, temporales de compilación/evaluación y sobrecostes del asignador. No se llamarán RSS, heap total ni memoria medida del proceso. Un programa One tiene payload de pares cero, pero sigue teniendo metadatos, una raíz y coste numérico.

Las fixtures observadas conservan V=614,676,676; si las expansiones permanecen iguales, sus tablas tendrán M=613,675,675. Se comprobará esa correspondencia frente a la revisión de dependencia finalmente seleccionada, sin atribuirle por sí sola una mejora de velocidad. El procedimiento ciclomático del cambio previo se repetirá sobre las nuevas fuentes con el mismo analizador y alcance, sin umbrales de aceptación nuevos.

**Alternativas:** afirmar ahorro global a partir de los tipos ignoraría memoria auxiliar; instrumentar el asignador o crear benchmarks de rendimiento ampliaría el objetivo de este incremento. Los datos del payload bastan para verificar la representación elegida.

## Risks / Trade-offs

- [Confundir fila k con nodo k] -> Documentar el desplazamiento +1 y probar diagnósticos, raíces, One sin filas y exclusión de nodos no alcanzables.
- [Regresión de presupuesto por One implícito] -> Mantener presencia perezosa y pruebas de límites 0, 1 y el tamaño lógico exacto en las tres etapas.
- [Pérdida de compartición] -> Probar 20000 niveles de `ti=Magma(ti-1,ti-1)` sin evaluación; exigir crecimiento lineal de filas y profundidad correcta.
- [Firma de inspección incompatible] -> Migrar todos los consumidores y doctests en un paso compilable antes de cambiar el almacenamiento; documentar la ruptura aceptada.
- [Cachés o conversiones que anulan la compactación] -> Revisar campos y rutas de inspección/compilación/evaluación: ninguna colección completa de nodos etiquetados dentro del núcleo final.
- [Confundir copia modificable con programa mutable] -> Control positivo de copia independiente y controles negativos de escritura en vistas/campos privados.
- [Dependencia todavía incompleta] -> Detener la aplicación hasta que `formalize-rust-core-contracts` esté completado y verificado; no absorber sus tareas restantes aquí.
- [Deriva numérica] -> Reutilizar las operaciones y su orden, y verificar tolerancias originales, sin exigir identidad bit a bit entre llamadas de trascendentes.

## Migration Plan

1. Completar la dependencia y registrar su revisión, resultados y API efectiva; preservar el trabajo previo pendiente de commit al retomar su propio flujo.
2. Migrar la inspección pública a valores/iteradores sobre la representación actual y adaptar consumidores, docs y lector privado del compilador. Este paso temporal todavía usa el almacenamiento etiquetado y no acredita el contrato final de compactación.
3. Sustituir la arena por `PairTable` y claves de internado de pares; después migrar el snapshot y la salida del compilador, con pruebas por etapa.
4. Cambiar la evaluación al recorrido directo de filas, reforzar pruebas de correspondencia, inmutabilidad y crecimiento lineal.
5. Actualizar demostración y guías con el payload y límites de interpretación; ejecutar regresiones y actualizar la evidencia ciclomática sobre fuentes identificadas.

Cada tarea verificada se registra en un commit de `develop` antes de avanzar y se publica en `origin/develop`; el merge a `main` se revisa con el usuario. No hay datos persistidos que migrar. Una reversión restaura conjuntamente almacenamiento, firmas y consumidores de este cambio, preservando la base de contratos completada. Los artefactos presentes solo autorizan y describen planificación: la implementación empieza con una nueva petición de apply.
