# Tasks

## 1. Base verificada

- [x] 1.1 Confirmar en `develop` la base completada de los tres cambios anteriores; registrar HEAD, estado de trabajo, Rust/target y ejecutar `cargo check --all-targets`, `cargo test --release` y `cargo run --release --example infer`. Verificar las tolerancias y contratos existentes y guardar la evidencia de partida; detener el apply si la dependencia no está disponible o falla.

## 2. Checkpoint, formato y referencias reproducibles

- [x] 2.1 Fijar revisiones y dependencias de desarrollo y preparar el checkpoint original y tok512 desde las URLs de `design.md`, registrando SHA-256 y formas reales. Generar y persistir los prefijos de 8/16 IDs del texto fijado antes de ejecutar EML; verificar BOS, posiciones, configuración esperada, alias y un rechazo reproducible al alterar el hash. Documentar la receta de preparación en `docs/tiny-model.md`.
- [x] 2.2 Implementar `tools/import_model.py` y documentar el formato canónico v1 byte a byte, incluidos límites, endian, catálogo, alias y constantes auxiliares. Verificar igualdad de bits de todos los pesos contra el origen y dos exportaciones con contenido idéntico; añadir fixtures pequeñas y comprobaciones de formato/alias que no requieran descargar el modelo.
- [x] 2.3 Añadir `src/model_io.rs` con tipos validados, metadatos escalares por bits y límites explícitos, usando solo `std`. Verificar round-trip con el exportador, formas correctas y rechazos de truncamiento, versión, NaN/infinito, alias, offsets, desbordamiento de tamaños y exceso de carga antes de construir nodos; documentar API y errores junto al formato.
- [x] 2.4 Implementar `tools/reference.py` con referencia original F32 y fórmulas explícitas F32/F64 independientes de EML; verificar la versión explícita F32 contra el original y auditar dtype de todas las etapas, evitando conversiones ocultas a F32 en la ruta F64. Generar trazas de ambos prefijos y el corpus determinista de escalares/productos/reducciones descrito en el diseño, con hashes y procedencia por tensor/índice.
- [x] 2.5 Completar la documentación de datos y referencias con comandos, versiones, política de precisión y criterios de comparación; reproducir desde los archivos fijados ambas referencias y comprobar que producen 512 logits finitos por prefijo, entradas idénticas y trazas identificadas. Registrar diferencias F32/F64 como evidencia, no como resultados EML.

## 3. Planificacion de memoria y ejecucion equivalente

- [x] 3.1 Añadir `execution/plan.rs` y su API privada/validada de instrucciones, plan prestado del programa, límites y estadísticas; implementar último uso, raíces vivas, One reservado y asignación determinista sin evaluar nodos. Verificar One, límites cero/exactos, mapa de raíces, plan de un programa numéricamente inválido y doctests que impidan fabricar o mutar planes.
- [x] 3.2 Añadir `execution/run.rs` y compartir internamente el kernel de `eval.rs`, preservando la API del evaluador original. Verificar lecturas de ambos operandos antes de escribir, raíces repetidas, comparación diferencial, presupuesto lógico antes de ejecutar y categorías/nodos de errores de dominio y desbordamiento; documentar firmas, ownership y semántica.
- [x] 3.3 Completar pruebas de liveness con operandos iguales, raíz intermedia, diamante, último uso simultáneo, fallos tras reutilización, reintentos y ejecuciones intercaladas. Verificar preservación de nodos/pares/raíces/estadísticas y la cadena estable de 10000 pasos con como máximo dos posiciones y error diferencial dentro de `1e-12 + 1e-12*abs(ref)`.
- [x] 3.4 Publicar estadísticas y una comparación reproducible del plan frente al evaluador original sobre la cadena y las tres redes existentes; verificar tamaños reales de instrucciones, programa, raíces, valores y capacidades sin atribuir payload a RSS. Actualizar `docs/architecture.md` y `docs/complexity.md` con API, límites y evidencia del ahorro de posiciones, incluyendo cualquier aumento del almacenamiento total.

## 4. Sintesis ampliada y puerta de viabilidad

- [x] 4.1 Documentar en `docs/synthesis.md` la decodificación diádica y las identidades/dominios de los candidatos One/Magma para constantes y operaciones del modelo, incluyendo agrupación de reducciones y pivote de softmax. Reproducir el caso de suma 7+0 con un DAG inspeccionable y registrar su fallo real; distinguir derivaciones algebraicas, dominios propuestos y evidencia F64, sin considerar resuelta la estabilidad por disponer de una identidad.
- [x] 4.2 Implementar la frontera de escalares y síntesis en `synth/scalar.rs`, conservando `Rational` y sus errores. Verificar todos los valores distintos requeridos del corpus con tolerancia `1e-9 + 1e-7*abs(ref)`, pureza One/Magma, procedencia, límites y diagnósticos por bits/ubicación; incorporar fixtures pequeñas de signos, cero, magnitudes pequeñas y límites y actualizar la documentación con el dominio efectivamente verificado. Si falla un valor requerido, pausar sin marcar esta tarea completa.
- [x] 4.3 Implementar en `synth/model.rs` las expansiones de suma/producto con signo, negación, recíproco positivo, raíz inversa, exponencial y SiLU necesarias, sin cambiar el kernel ni evaluar durante construcción. Verificar identidades documentadas, errores y el corpus de operandos contra F64 con `1e-6 + 1e-5*abs(ref)`, además de todas las tolerancias escalares y neuronales originales; registrar cobertura, nodos y límites numéricos.
- [ ] 4.4 Ejecutar la puerta de viabilidad con productos escalares de longitudes 8, 64 y MLP y fórmulas representativas de RMSNorm, softmax y SwiGLU construidas desde los datos fijados. Verificar todas las tolerancias del nivel correspondiente, medir V/E/R/D, posiciones y memoria, y estimar el crecimiento completo frente a límites explícitos; publicar el informe en `docs/tiny-model.md`. Marcar completa solo si todos los casos pasan y el siguiente hito cabe en los límites; cualquier fallo bloquea las secciones dependientes y requiere revisión con el usuario.

## 5. Constructores tensoriales

- [ ] 5.1 Añadir formas y tensores de handles en `src/tensor.rs`, con orden row-major y operaciones estructurales de indexación/reshape. Verificar dimensiones, productos de tamaños con overflow, procedencia de handles y ausencia de evaluación numérica, y documentar invariantes y errores de esta frontera.
- [ ] 5.2 Añadir selección de embedding, proyecciones, reducciones de orden fijo, producto elemento a elemento y residuales usando las expansiones verificadas; reutilizar constantes por bits solo dentro de su arena. Verificar formas, alias, tokens fuera de rango, presupuestos y resultados de fixtures pequeñas y corpus real contra F64, inspeccionando que solo se generan One/Magma.
- [ ] 5.3 Construir RMSNorm y SwiGLU con el epsilon y política de precisión fijados. Verificar componentes aislados contra F64 con `1e-6 + 1e-5*abs(ref)`, casos de cancelación del corpus y propagación de errores; añadir pruebas y derivaciones/documentación de la composición.
- [ ] 5.4 Construir RoPE por pares y atención causal agrupada con ocho cabezas Q y cuatro KV, excluyendo términos futuros y usando el pivote simbólico de softmax. Verificar rotaciones, emparejamiento Q/KV, normalización, invariancia ante un token futuro y tolerancia de componentes, sin infinitos ni operaciones nativas ocultas.
- [ ] 5.5 Completar la guía de constructores tensoriales en `docs/tiny-model.md`, con shapes, orden de reducciones y ejemplos pequeños; ejecutar los ejemplos y la suite tensorial con ambos evaluadores y comprobar presupuestos, diagnóstico lógico y los criterios numéricos definidos.

## 6. Bloque y modelo completos

- [ ] 6.1 Añadir `models/llama.rs` con configuración validada, constructor de un bloque y mapa de salidas diagnósticas por etapa/posición/coordenada. Comparar el primer bloque para ambos prefijos con las trazas F64 usando `1e-4 + 1e-4*abs(ref)`; verificar errores de configuración/entrada y documentar que los pesos son propios de cada bloque.
- [ ] 6.2 Ensamblar embedding, cinco bloques, normalización final y proyección compartida, y añadir `examples/infer_tiny.rs`. Ejecutar ambos prefijos con los dos evaluadores, verificando todas las etapas, los 512 logits contra F32 con `1e-4 + 1e-4*abs(ref)` y equivalencia diferencial EML; comprobar pureza, snapshots después de liberar importación/arena y que un fallo no entrega logits parciales. Conservar capturas diagnósticas separadas de los programas con solo logits como raíces.
- [ ] 6.3 Documentar en README y `docs/tiny-model.md` los comandos reales de importación e inferencia, límites usados, identidad de programas, errores por etapa y resultados verificados. Reproducir los comandos para 8 y 16 tokens y revisar enlaces y ejemplos; identificar explícitamente qué costes corresponden a un prompt nuevo y cuáles a un programa ya preparado.

## 7. Adaptadores y comparacion entre motores

- [ ] 7.1 Definir el protocolo versionado de adaptadores y añadir los modos EML original/planificado y PyTorch F32 con entradas por IDs, logits completos y tiempos por fase. Verificar correspondencia exacta de inputs, categorías de fallo y precisión/threading efectivos; documentar el protocolo y un ejemplo reproducible en `docs/benchmarks.md`.
- [ ] 7.2 Añadir el adaptador llama2.c de revisión fijada, con CPU/un hilo, pesos F32 y estado de atención reiniciado entre corridas. Verificar 512 logits de ambos prefijos contra la referencia, ausencia de reutilización de prefijos y exactitud de IDs; registrar receta de compilación y el coste de proyectar logits en cada posición de su recorrido secuencial.
- [ ] 7.3 Fijar revisión y conversión F32 para llama.cpp, verificar todos los tensores/alias/layout RoPE y vocabulario contra el original y añadir el adaptador de IDs/logits con GPU desactivada y un hilo. Verificar ambos prefijos dentro de tolerancia y un rechazo al alterar conversión o mapeo; documentar versiones, comandos y precisión de cachés realmente utilizados.
- [ ] 7.4 Implementar `tools/benchmark.py` con tres calentamientos y diez repeticiones mínimas, reinicio de estado, muestras/mediana/p95, fases separadas, extremo a extremo y memoria con método/unidad/alcance. Verificar el cálculo de métricas y clasificación de una divergencia, dependencia ausente, timeout y métrica no disponible; generar JSON versionado y Markdown coherente y documentar exclusiones y política de repetición.
- [ ] 7.5 Ejecutar la comparación de aceptación de ambos prefijos en los cinco modos, conservando hashes, comandos, versiones, muestras, logits y memoria. Verificar tolerancia elemento a elemento en las diez combinaciones y publicar resultados reproducibles en `docs/benchmarks.md`; marcar completa solo con todos los modos válidos, aunque EML no gane en tiempo o memoria.

## 8. Integracion y cierre

- [ ] 8.1 Ejecutar `cargo check --all-targets`, `cargo test --release`, `cargo test --doc -- --show-output`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps` y la demostración original; ejecutar también las verificaciones externas explícitas del modelo y herramientas documentadas. Confirmar que `Cargo.toml` mantiene el runtime sin dependencias, que los contratos originales siguen pasando y que ninguna prueba omitida se cuenta como aceptación del modelo.
- [ ] 8.2 Revisar trazabilidad entre las cuatro specs, sus escenarios y la evidencia final, comprobar enlaces/comandos documentados y ejecutar `openspec validate benchmark-tiny-model-eml --strict`. Confirmar con apply instructions que todas las tareas efectivamente implementadas están marcadas y que no queda ninguna puerta, fixture o motor requerido pendiente antes de declarar completado el cambio.

## Flujo de ejecucion

1. Trabajar en `develop`, después de las dependencias verificadas en 1.1. La base documental puede seguir pendiente de revisión/archivo sin que ello autorice mezclar sus cambios.
2. Completar y verificar cada tarea antes de marcarla; crear un commit con sus cambios y su casilla antes de avanzar a la siguiente.
3. Usar mensajes `<tipo>(benchmark-tiny-model-eml): tarea <X.Y> - <resumen>` y publicar los avances en `origin/develop`.
4. Al cerrar o pausar un apply con avances, crear o actualizar la PR `develop` hacia `main` y entregar su enlace para revisión. Comprobar la PR vigente; no asumir que una PR anterior sigue abierta.
5. La sección 3 es independiente de resolver la nueva aritmética, pero el orden del checklist se mantiene para poder verificar cada commit. Las secciones 5, 6 y la aceptación completa de 7 dependen de 4.4.
6. Una comprobación fallida conserva su tarea y dependientes pendientes. No se cambia el modelo, el prefijo, el número de capas, los thresholds ni la semántica para completar una casilla; presentar el bloqueo y solicitar revisión del diseño si hace falta.

## Workflow follow-up

- Obtener revisión del usuario antes de merge y archivo.
- Archivar en orden de dependencia cuando se autorice; el archivo y la sincronización de especificaciones principales no son tareas de esta implementación.

## Evidencia de partida (1.1)

- HEAD: `6cb85ec4beaeda202d8c8a62aa656ca836002424`, rama `develop`, sin cambios rastreados al iniciar; únicamente los artefactos nuevos de este cambio estaban sin seguimiento.
- Dependencias: `implement-one-magma-eml` 16/16, `formalize-rust-core-contracts` 23/23 y `use-internal-pair-tables` 15/15, todas completas según el inventario actual.
- Rust `1.92.0 (ded5c06cf 2025-12-08)`, LLVM 21.1.3, target `aarch64-apple-darwin`.
- `cargo check --all-targets`, `cargo test --release` y `cargo run --release --example infer`: satisfactorios. Pasaron 37 pruebas y 15 doctests, sin omisiones.
- Red original: V/E/R/D `(614,1226,1,148)`, `(676,1350,1,148)` y `(676,1350,1,150)`; errores `2.426e-14`, `9.104e-15` y `4.829e-15`, bajo `1e-6`. Se preservan las pruebas escalares de `1e-9` y `1e-8`.
- Evidencia local: `target/analysis/tiny-baseline-20261113T010313Z/`, con comandos, códigos de salida, logs, estado Git y versiones.
- La PR #3 fue comprobada como MERGED al iniciar este apply; hará falta una nueva PR para los avances de este cambio.

## Evidencia de entradas (2.1)

- Preparador `tools/prepare_tiny.py`, fuentes con SHA-256 fijados, lock de 12 paquetes con hashes y fixtures de IDs versionadas. Entorno aislado: CPython 3.12.12, PyTorch 2.9.1, NumPy 2.2.6 y SentencePiece 0.2.1.
- Checkpoint inspeccionado: 260032 parámetros únicos, cinco bloques, dimensión 64, MLP 172, cabezas Q/KV 8/4, vocabulario/contexto 512/512 y alias embedding/salida comprobado por storage, offset y bits.
- Dos preparaciones offline producen metadata e inputs idénticos; corrupción del `.pt` del mismo tamaño rechazada con código 1 y sin salida parcial. Dos pruebas locales satisfactorias; receta y hashes del resultado en `docs/tiny-model.md`, evidencia en `target/analysis/tiny-input-preparation/`.

## Evidencia de exportacion (2.2)

- `tools/import_model.py` produce EMLTINY v1 documentado byte a byte y conserva el alias de salida. Los 47 tensores de pesos propietarios coinciden por bits con `.pt` y `.bin`.
- Paquete: 52 entradas, 1043208 bytes, SHA-256 `434011a9e90db0bd15b39b80f11676c9b238b27ca8c526a9b0b87efc5b02ecd5`. Dos exportaciones completas idénticas; metadata de constantes y procedencia en el manifiesto.
- Seis pruebas Python satisfactorias, incluyendo fixture binaria de 176 bytes independiente del modelo. Evidencia local: `target/analysis/tiny-canonical-export/`.

## Evidencia del lector Rust (2.3)

- `src/model_io.rs` separa contenedor y stories260K validados, con payload prestado, alias sin copia, bits F32 finitos, límites explícitos y errores localizados. API y categorías en `docs/tiny-model-format.md`.
- `tools/verify_model_io.py` compara 52 entradas y 292930 escalares (incluidos alias) del paquete real con el lector Rust: todos los bits y metadatos idénticos; SHA-256 canónico sin cambios.
- Cinco pruebas locales del lector cubren los 176 truncamientos de la fixture, límites, formas/inventario, constantes, alias/cadenas, offsets, overflow y NaN/inf. `cargo test --release`: 42 pruebas y 16 doctests satisfactorios; `cargo clippy --all-targets -- -D warnings` satisfactorio.

## Evidencia de referencias (2.4)

- `tools/reference.py` verifica 93 etapas originales contra F32 explícito y audita 213 etapas explícitas F32/F64 por prefijo. Todas finitas; 512 logits por recorrido/prefijo; cero divergencias con el criterio de etapa. Diez pruebas Python locales satisfactorias.
- Fuente original intacta fijada por hash en `tools/reference.sources.json`; atención manual/eval/CPU/un hilo. Trazas con hashes por tensor e índice causal documentado en `docs/tiny-model.md`.
- Corpus fijado: 280688 escalares distintos, 6599 operaciones, 631 reducciones completas (8/64/172) y 208 componentes. SHA-256 `d4c9207a3ea4e9b526bdc4c6b2a791cd6439586c853a8b58e6c3f9c2095677c5`; ubicación `target/tiny-model/reference/corpus/`.

## Evidencia de reproduccion (2.5)

- Reejecución offline en `target/analysis/tiny-reference-repeat/`; `tools/verify_reference.py` acredita 16 archivos canónicos idénticos, 1038 etapas y 388939 referencias de procedencia. Reporte SHA-256 `a853b59e12d26d3a9517d17c489069dd0bef864ebaf4d0859b6e84066771ca3f`.
- Ambos prefijos tienen 512 logits finitos en las tres rutas. Máximos F64/original F32: `5.659565e-6` (8) y `5.016644e-6` (16), sin divergencias. Política, criterios, comandos, entorno y rango real del corpus en `docs/tiny-model.md`.

## Evidencia de planificacion (3.1)

- `execution::plan` deriva instrucciones privadas de tres índices, presta el programa y conserva raíces/One mediante últimos usos y asignación determinista. Límites de nodos/posiciones separados, reserva falible y estadísticas de payload/capacidad.
- Tres pruebas específicas satisfactorias: One y límites 0/exactos, raíces vivas/duplicadas y planificación sin evaluar de un programa que desborda. Pasaron 19 doctests; nuevos controles negativos E0451/E0594/E0505 acreditan privacidad, inmutabilidad y préstamo. Clippy de todos los targets sin advertencias.

## Evidencia de ejecucion planificada (3.2)

- `execution::run` posee buffer por llamada y comparte `eval::magma_value` sin cambiar sus comprobaciones ni el evaluador original. Ambos operandos se cargan antes de escribir; fila i conserva diagnóstico lógico i+1.
- Cinco pruebas de ejecución/plan satisfactorias: coincidencia diferencial, raíces repetidas/One, sobrescritura del operando repetido, presupuestos lógicos y mismos errores/nodos de dominio y overflow. Suite release: 47 pruebas y 20 doctests; Clippy de todos los targets satisfactorio.

## Evidencia de liveness (3.3)

- Nueve pruebas específicas satisfactorias; añadidos diamante/último uso simultáneo, raíces intermedias, errores intercalados/reintentos y 32 DAGs compartidos deterministas. Un oráculo de identidades lógicas comprueba las lecturas independientemente de los valores numéricos.
- Cadena estable de 10000 pasos: 10001 nodos, dos posiciones (16 bytes), equivalencia bajo `1e-12 + 1e-12*abs(ref)`, rechazo con una posición. Nodos, pares, raíces y estadísticas permanecen intactos; Clippy sin advertencias.

## Evidencia de almacenamiento (3.4)

- `Program::storage` y variantes `evaluate_with_storage`/`run_with_storage` publican tamaños y capacidades reales. `examples/plan_memory.rs` compara cadena y tres redes, con todas las salidas verificadas.
- Posiciones planificadas: 2/24/25/25 frente a 10001/614/676/676. Las instrucciones de 24 bytes aumentan el total contabilizado: por ejemplo, capacidad+metadata de cadena 342304 → 502384 bytes. RSS no medido; alcance y tablas completos en `docs/complexity.md` y API en `docs/architecture.md`.
- Captura repetida con cifras idénticas y error diferencial cero en `target/analysis/execution-plan-memory/`. Suite release: 52 pruebas y 20 doctests; Clippy satisfactorio.

## Evidencia de derivaciones y contraejemplo (4.1)

- `docs/synthesis.md` documenta F32 diádico, Horner fraccionario y escala H/D con primer operando fijo ±ln(2), signos, recíproco, raíz inversa, SiLU, reducciones balanceadas y pivote simbólico. Se distinguen dominios reales de validación F64 pendiente.
- `cargo run --release --example synthesis_probe -- --dump`: entradas [7,0], programa V/E/R/D=40/78/1/37, cuatro posiciones; ambos evaluadores fallan con `ExponentialOverflow` en el nodo lógico 38 `Magma(37,0)`. DAG completo inspeccionable; Clippy satisfactorio. Este fallo histórico esperado no acredita las candidatas nuevas.

## Evidencia de sintesis escalar (4.2)

- `synth/scalar.rs` implementa decodificación entera diádica, mantisa fraccionaria y H/D; conserva `Rational`. Fixtures de signos, cero, magnitudes, límites exactos y procedencia satisfactorias.
- `tools/verify_scalar_corpus.py`: **280688/280688** valores pasan con ambos evaluadores, pureza y tolerancias fijadas. Máximos V=829, D=823, posiciones=8; fracción máxima del umbral `3.126478192134047e-7`; 12.685 s. Sin valores excluidos ni fallidos. Evidencia: `target/analysis/scalar-synthesis/report.json`.
- Suite release: 55 pruebas y 20 doctests; Clippy y documentación con advertencias denegadas satisfactorios. Dominio concreto acreditado y límites documentados; operaciones compuestas pendientes.

## Bloqueo histórico de operaciones (4.3 inicial, revisión 3a277e0)

- Los candidatos iniciales en `synth/model.rs` fallaron en el producto del caso 10, bits `0x3f8afc5f` en ambos operandos, `prefix8/explicit_f32/embedding[301]`, cuadrado requerido por RMSNorm.
- Valor exacto `1.0858267545700073`, producto esperado `1.179019740940035`; ambos evaluadores devuelven `ExponentialOverflow` en nodo lógico 312 `Magma(311,0)`. V/E/R/D=356/710/1/301, diez posiciones, sin agotar límites de 100000 nodos/posiciones.
- Diez casos pasaron, uno falló, 6588 quedaron sin verificar. Reproducción en esa revisión: `cargo run --release --example verify_operations -- tests/fixtures/tiny-operation-blocker.txt` (salida 1). Evidencia histórica con procedencia/hashes: `target/analysis/operation-synthesis/report.json`.
- También falló la prueba candidata local de SiLU para x=1.5 (nodo 210). Las 18 pruebas de síntesis original/red/ejecución pasaron; Clippy satisfactorio. El usuario autorizó revisar las expansiones conservando exp/ln y tolerancias.

## Evidencia de operaciones revisadas (4.3)

- Suma desplazada con centro 128, negación por reflexiones y producto normalizado por 64 con doce duplicaciones; raíz inversa con escala fija 1/2 y denominador SiLU mediante Magma directo. Identidades, dominios y constantes por bits documentados en `docs/synthesis.md`; sin evaluar en construcción ni modificar el kernel.
- **6599/6599 operaciones** pasan con ambos evaluadores sobre el corpus original. Máximos V/E/R/D=1013/2024/1/569, 17 posiciones; máxima fracción de umbral `0.00025848092631870485`, diferencial cero. Límites originales de 100000 nodos/posiciones conservados. Evidencia nueva: `target/analysis/operation-synthesis-revised/report.json`.
- La fixture del cuadrado y la prueba SiLU x=1.5 ahora pasan. Añadida regresión de signos, cancelación y desbordamientos históricos. Suite: 58 pruebas Rust y 20 doctests satisfactorios; Clippy satisfactorio. La puerta 4.4 debe verificar las composiciones completas y su coste antes de tensores/modelo.

## Bloqueo histórico de composiciones (4.4, revisión f7c5fda)

- `examples/verify_components.rs` y `tools/verify_component_corpus.py` construyen un DAG cerrado por caso, con reducción balanceada, RMSNorm, softmax de pivote 0 y SwiGLU. Verifican los 631 productos escalares y 208 componentes originales; añaden 62 filas completas gate/up seleccionadas desde ubicaciones ya fijadas del corpus. Los 901 casos se persisten antes de EML; hashes y procedencia en `target/analysis/component-synthesis/`.
- Pasan **648 casos**: los 631 productos (70 de longitud 8, 490 de 64 y 71 de 172), seis RMSNorm y once softmax. Máxima fracción del umbral `0.00419606113722238`, diferencial cero. Fallo en índice **648**, `prefix8/explicit_f32/layers.0.softmax[5,7]`: ambos evaluadores devuelven `ExponentialOverflow` en nodo **1596**, `Magma(1595,0)`; V/E/R/D=3657/7312/8/484, 26 posiciones. Quedan **252 casos sin verificar**, incluidos todos los SwiGLU completos.
- La exponencial de un score desplazado vale ≈3287; la suma desplazada intenta volver a exponenciarla. Reproducción local: `cargo run --release --example verify_components -- tests/fixtures/tiny-softmax-blocker.txt` (salida 1). El caso completo y el recorrido del corpus coinciden; no se altera el pivote ni el kernel.
- Máximos preparados V=107448, E=214894, R=64, D=1115, posiciones=336. Capacidad+metadata máxima por caso exitoso: original 2956888 bytes, planificado 4677400. RSS del proceso completo de verificación: 31358976 bytes mediante `/usr/bin/time -l` (macOS). Límites por caso: 1000000 nodos y 100000 posiciones; no se agotaron.
- La extrapolación MAC-only da ≈1.32/2.66 mil millones de nodos para 8/16 tokens, por encima de los presupuestos del siguiente hito (10000000 nodos, 1000000 posiciones, 2 GiB). No es cota ni memoria medida: comparte menos constantes que un modelo ensamblado y omite costes no lineales/temporales; debe refinarse. Informe y supuestos en `docs/tiny-model.md`.
- Dos pruebas locales adicionales del ejemplo pasan; Clippy satisfactorio. La casilla 4.4 permanece pendiente. Apply pausado en **13/29 tareas completas** para revisar softmax y coste estructural; integración dependiente pendiente.

## Revisión de composiciones y bloqueo de recursos (4.4 pendiente)

- El usuario autorizó revisar suma de softmax y coste manteniendo exp/ln. `add_positive` usa suma en dominio logarítmico con `M(t,E(-1))=exp(t)+1`; `divide_positive` normaliza sin producto firmado de grandes exponenciales. El pivote 0, el árbol de reducción, los datos y tolerancias se conservan. El producto recupera escala con `4096*(q+1/8)-512` en vez de doce duplicaciones.
- **6599/6599 operaciones** vuelven a pasar: V máximo 910, D máximo 441, posiciones 19, fracción máxima del umbral `2.176523734359192e-5`. Evidencia: `target/analysis/operation-synthesis-rescaled/`.
- **901/901 composiciones** pasan con ambos evaluadores: 631 productos escalares, 48 RMSNorm, 160 softmax y 62 SwiGLU. Máximos V/E/R/D=86790/173578/172/627, posiciones=341; fracción máxima `0.0020050764041026478`, diferencial cero. RSS del proceso de verificación: 31670272 bytes. Evidencia: `target/analysis/component-synthesis-log-domain/`, con los mismos hashes de datos y protocolo.
- `synthesis_cost` mide costes con constantes/transformaciones compartidas: 3969 productos a 24 nodos nuevos cada uno y 3968 sumas a 15; repetir un par añade cero. La estimación ampliada da 86952895/169572207 nodos para 8/16 tokens, excluida la síntesis completa de pesos, que no está disponible. Los supuestos, familias y payloads están en `docs/tiny-model.md`.
- **Bloqueo real de construcción:** los 47 tensores de pesos tienen 259577 patrones distintos; se alcanza `NodeLimit { limit: 10000000 }` tras 106386. Primer valor incompleto: `layers.1.feed_forward.w2.weight[28,104]`, índice 4920, bits `0x3d14ebf3`. El mismo escalar aislado pasa con V=297, D=291 y seis posiciones. El caso y contadores se repiten; RSS del intento completo de medición 1003044864 bytes, antes de compilar/planificar los pesos. Evidencia: `target/analysis/synthesis-shared-cost-confirmed/`.
- Pasan 60 pruebas Rust, 20 doctests y tres pruebas del ejemplo de componentes; Clippy y rustdoc sin advertencias. La 4.4 sigue pendiente porque exige tanto corrección como cabida en los presupuestos fijados. Apply pausado en **13/29** para revisar síntesis de constantes y coste estructural.
