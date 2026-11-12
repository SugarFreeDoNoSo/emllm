# Tasks

## 1. Dependencia y revision de partida

- [ ] 1.1 Confirmar que `formalize-rust-core-contracts` y su base están completados y verificados; registrar la revisión de partida, los resultados de contratos y fixtures, V/E/R/D y la procedencia del analizador de esa revisión. Si falta esa dependencia o sus comprobaciones fallan, detener este apply e informar el bloqueo sin absorber sus tareas pendientes.

## 2. Migracion compilable de la inspeccion

- [ ] 2.1 Cambiar `Arena::nodes()` y `Program::nodes()` a iteradores de valores `Node`, añadir `node_count()` y consulta por valor en ambos tipos y adaptar el lector privado del compilador sin materializar un vector de entrada; migrar todos los consumidores, ejemplos y documentación afectada, verificando `cargo check --all-targets`, las pruebas existentes y los doctests con la representación temporal aún etiquetada.
- [ ] 2.2 Añadir pruebas de consulta e iteración por valor para arena vacía y los tres nodos One, Magma(0,0), Magma(1,1); verificar longitud exacta y decreciente del iterador, ausencia fuera de rango incluido `usize::MAX` e independencia de una copia modificada. Revisar las rutas de consulta para comprobar que no asignan una colección de nodos y documentar el contrato de copia.

## 3. Arena con pares e identidad preservada

- [ ] 3.1 Introducir el almacenamiento privado `PairTable` en `expr` y migrar la arena e internado a pares ordenados, con One perezoso y `pairs()` de solo lectura; documentar la correspondencia fila k/nodo k+1 y verificar arena vacía, One sin filas, tabla [[0,0],[1,1]], internado repetido y operandos invertidos.
- [ ] 3.2 Verificar presupuestos de construcción 0, 1 y límite exacto, reutilización en arena llena, errores de síntesis sin raíz parcial y procedencia de handles con índice 0 entre arenas, incluso después de liberar su origen; revisar doctests de referencias opacas y de escritura prohibida en la vista de pares de arena, con controles positivos y diagnósticos válidos.

## 4. Snapshot y compilacion de tablas

- [ ] 4.1 Migrar `Program` y la salida del compilador a la tabla privada alcanzable, sin conservar `Vec<Node>` como almacenamiento; mantener V/E/R/D y publicar `pairs()` de solo lectura. Actualizar la documentación y doctests de campos privados al layout real; verificar los cuatro casos de estadísticas de la especificación y que One exige presupuesto uno aunque tenga cero filas.
- [ ] 4.2 Verificar remapeo con filas no alcanzables y raíces [v,One,v], esperando tabla [[0,0],[1,1]] y raíces [2,0,2]; cubrir raíces vacías/ajenas, límite insuficiente y las fixtures internas de referencia inexistente, autorreferencia y ciclo entre filas definidas en `design.md`, comprobando categorías e índices lógicos sin programas parciales.
- [ ] 4.3 Añadir la cadena compartida de 20000 niveles `ti=Magma(ti-1,ti-1)` y verificar 20000 filas, V=20001, E=40000 y D=20000 mediante compilación e inspección iterativas sin evaluación; revisar y documentar que las rutas finales del núcleo no expanden subárboles ni materializan colecciones completas de nodos etiquetados.
- [ ] 4.4 Verificar estabilidad de pares, raíces, copias de nodos y estadísticas después de ampliar/liberar la arena; añadir doctests positivos y negativos de la vista de pares del programa, raíces y fabricación de snapshots. Ejecutarlos con diagnósticos visibles y actualizar `docs/architecture.md` para distinguir vista lógica, filas residentes e identificadores de diagnóstico.

## 5. Evaluacion directa y contratos numericos

- [ ] 5.1 Adaptar `evaluate` al recorrido directo de pares, inicializando One tras comprobar el presupuesto lógico y usando fila+1 para diagnósticos; documentar esta correspondencia y verificar One, Magma(One,One), raíces [1,0,1], presupuesto cero con tabla vacía y los errores de dominio/desbordamiento de la base sin cambiar la semántica numérica.
- [ ] 5.2 Verificar que el fallo de una dependencia entre varias raíces devuelve un error resoluble por inspección sin salidas parciales; ejecutar evaluaciones intercaladas y reintentos de presupuesto, comprobando preservación de pares/raíces/estadísticas y tolerancias. Revisar que `f64` sigue limitado a los valores de evaluación y que no se introducen clamps, reconocimiento de expansiones ni referencia convencional en producción.

## 6. Evidencia de almacenamiento y guia de migracion

- [ ] 6.1 Ampliar la demostración con `pair_rows`, `pair_payload_bytes` y `tagged_node_payload_model_bytes`, manteniendo estadísticas y errores; ejecutar las tres fixtures en su orden original y comprobar tolerancia 1e-6 y las fórmulas de payload usando los tamaños reales del target. Registrar en `docs/complexity.md` revisión, comando, Rust/target, tamaños de tipos y exclusiones de capacidad, metadatos, raíces, internado y temporales.
- [ ] 6.2 Completar la guía de migración de firmas en README y `docs/architecture.md` y enlazar la evidencia de almacenamiento; verificar que todos los nombres de API y ejemplos corresponden a los entregables y que la explicación distingue One implícito, coste lógico, payload estructural, memoria total y precisión numérica, sin promesas de aceleración.

## 7. Integracion y cierre de implementacion

- [ ] 7.1 Ejecutar `cargo test --release`, `cargo test --doc -- --show-output`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo run --release --example infer`; verificar regresiones de procedencia, snapshots, errores, estadísticas, racionales, operaciones y red con las tolerancias originales y registrar resultados junto a la evidencia de almacenamiento.
- [ ] 7.2 Repetir el procedimiento ciclomático documentado por la dependencia con el mismo analizador y alcance, conservando fuentes/metadatos y JSON original; verificar cobertura y comparación de unidades sin doble conteo ni umbrales nuevos, documentando explícitamente cualquier medición no disponible o incompleta. Validar este cambio con `openspec validate use-internal-pair-tables --strict` y confirmar el seguimiento final mediante apply instructions.

## Flujo de ejecucion

1. Aplicar este cambio después de `formalize-rust-core-contracts`, sobre la misma rama `develop`.
2. Crear un commit por tarea completada y verificada, incluyendo sus cambios y la casilla correspondiente; completar el commit antes de avanzar a la siguiente tarea.
3. Identificar cambio y número de tarea: `<tipo>(use-internal-pair-tables): tarea <X.Y> - <resumen>`.
4. Publicar avances en `origin/develop` y revisar con el usuario antes del merge a `main`.
5. El cambio intermedio de inspección de la sección 2 no acredita por sí solo el almacenamiento final: completar la migración de ambas tablas y la evaluación antes de declarar cumplida la capacidad.

## Workflow follow-up

- Revisar la implementación y su evidencia con el usuario antes de archivar.
- Archivar los cambios en orden de dependencia; el archivo no forma parte de las tareas de implementación anteriores.
