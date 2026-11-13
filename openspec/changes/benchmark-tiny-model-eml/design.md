# Design

## Context

La motivación está en [proposal.md](proposal.md). La revisión inspeccionada es `6cb85ec4beaeda202d8c8a62aa656ca836002424`, en `develop`.

Hechos comprobados en el repositorio:

- `src/expr.rs` interna pares ordenados y mantiene procedencia de handles. `Program` posee un snapshot inmutable con One lógico en 0, sin fila física.
- `src/program.rs` compila iterativamente solo nodos alcanzables, con orden determinista. El snapshot ya está listo para lectura secuencial.
- `src/eval.rs` recorre los pares una vez, sin consultar un HashMap por nodo, pero retiene un `f64` por nodo lógico. Su kernel comprueba finitud, dominio y desbordamiento en un orden observable.
- `src/synth.rs` admite racionales normalizados con `abs(p)<=2` y `q<=2`. Las expansiones son identidades reales, no garantías generales de estabilidad `f64`.
- La expansión actual de suma requiere el intermedio `exp(exp(x)-x-y)`. Para 7 y 0 su exponente es aproximadamente 1089.63, mayor que el umbral finito de `f64`, aproximadamente 709.78. Es un contraejemplo algebraico identificado durante la exploración; todavía no es una fixture EML ejecutada.
- `src/network.rs` construye una expresión cerrada por entrada. No existen entradas enlazadas, tensores, importación de pesos ni sesión KV reutilizable.
- `Cargo.toml` no contiene dependencias. Las pruebas actuales cubren procedencia, snapshots, pares, errores, estadísticas y tolerancias escalares/neuronales.

No hay especificaciones principales publicadas. Se preservan los contratos de los tres cambios completados, especialmente `eml-expressions`, `eml-program-contracts` y `eml-pair-table-storage`. La ampliación de escalares usa una frontera nueva y no cambia el dominio de `Rational`.

## Goals / Non-Goals

**Goals:**

- Derivar un plan de memoria del mismo programa que consume el evaluador de referencia, para aislar el efecto de reutilizar temporales.
- Mantener una cadena trazable desde pesos/IDs hasta expresiones, activaciones y logits; distinguir errores de conversión, síntesis, modelo y ejecución.
- Obtener evidencia temprana con una prueba numérica específica y una comparación de buffers sobre grafos existentes antes del coste del Transformer completo.
- Usar constructores de bloques reutilizables en código, con pesos distintos por capa y sin confundir esa reutilización con compartir resultados numéricos.

**Non-Goals:**

- Un runtime tensorial alternativo, JIT o cambio de instrucciones del lenguaje.
- Un contrato universal de síntesis para todos los F32 finitos o todas las longitudes de contexto.
- Un porcentaje prometido de aceleración o reducción de memoria total. Los límites funcionales del hito están en la propuesta y las especificaciones.

## Decisions

### 1. Checkpoint original y dos prefijos fijados antes de EML

Fuente primaria: `karpathy/tinyllamas`, revisión `0bd21da7698eaf29a0d7de3992de8a46ef624add`, archivos `stories260K/stories260K.pt`, `stories260K/stories260K.bin`, `stories260K/tok512.model` y `stories260K/tok512.bin`. El importador fijará sus SHA-256 después de descargarlos y conservará URLs, tamaños y versiones de herramientas. Esta planificación no ha descargado ni ejecutado los pesos.

La documentación original confirma 5 bloques, dimensión 64, 8 cabezas Q, 4 KV y vocabulario 512. El código de referencia implica cabeza de dimensión 8, RoPE por pares con theta 10000 y epsilon 1e-5. La dimensión MLP esperada es 172, derivada del redondeo a múltiplo de 4; se comprobará contra las formas reales, igual que contexto y alias de embedding/salida. Una discrepancia bloquea la importación hasta explicar su origen.

Se tokenizará una sola vez el texto fijo `Once upon a time, there was a little girl named Lily. She loved to play outside in the park.` con tok512, BOS incluido y sin añadir EOS. Las fixtures son los primeros 8 y 16 IDs de esa misma lista, contando BOS. Se persistirán IDs, posiciones 0..T-1, texto, política y hashes antes de ejecutar EML. No se seleccionan entradas según su éxito numérico.

Se prefiere el original frente a asumir equivalencia de un GGUF publicado. El candidato `ggml-org/test-model-stories260K`, revisión observada `479896ec924af6d40fd419ab8f4d1eb2101de00d`, requiere comprobar tensores, tokenizer y metadata; una conversión propia fijada es válida si demuestra esa equivalencia.

### 2. Frontera de importación pequeña y tipada

`tools/import_model.py` hará las tareas dependientes de PyTorch/tokenizer y emitirá:

- Un manifiesto JSON con procedencia, hashes, configuración, alias, transformaciones, tipos y versiones.
- Un paquete binario canónico v1 para el lector Rust, con magic, versión, campos enteros little-endian, catálogo de tensores, formas, offsets, alias y payload de bits F32.
- Entradas fijas y coeficientes RoPE F32 para posiciones 0..15, también identificados y hasheados. Epsilon y escala de atención tendrán política de precisión explícita.

El formato se documentará byte a byte con el exportador y su lector; no será un lector genérico de safetensors/GGUF/Pickle dentro de la biblioteca. El catálogo usará tipos de tensor conocidos, longitudes acotadas y offsets/tamaños comprobados antes de reservar. `src/model_io.rs` recibirá bytes y límites explícitos y devolverá un tipo validado con datos inmutables. SHA-256 y la comprobación cruzada de archivos residen en las herramientas; Rust valida estructura, coherencia y contenido escalar antes de construir.

Los escalares se transportan como bits `u32`, no como hojas ejecutables. Su decodificación en signo, mantisa y exponente usa metadatos enteros. Los alias de pesos se conservan; no se duplica ni re-cuantiza una matriz al convertirla. Los datos grandes y capturas quedan bajo `target/`; las fixtures pequeñas de formato, manifiestos fijados y recetas reproducibles pueden versionarse.

Alternativa descartada para este hito: añadir dependencias de formatos, tokenizer y álgebra tensorial al núcleo Rust. No son necesarias para ejecutar un paquete ya verificado y ampliarían la frontera de compatibilidad.

### 3. Dos referencias externas con precisión explícita

La implementación PyTorch original de llama2.c, inspeccionada en `350e04fe35433e6d2941dce5a1f53308f87058eb`, es la referencia F32 del modelo entrenado. Se ejecutará en CPU, modo evaluación, un hilo, dropout desactivado y atención explícita reproducible, registrando cualquier adaptación necesaria para extraer trazas.

`tools/reference.py` proporcionará además una referencia matemática F64 independiente de los constructores EML: mismos pesos F32 elevados exactamente a F64, mismo orden de capas y las constantes posicionales F32 canónicas. Debe implementar las fórmulas en ese dtype de principio a fin: el original contiene conversiones `.float()` en RMSNorm, RoPE y softmax, por lo que llamar solamente a `.double()` no basta. Se verificará primero la versión explícita F32 contra la implementación original y se registrarán las diferencias F32/F64.

Las trazas incluyen embedding, RMSNorm, Q/K/V antes y después de RoPE, puntuaciones/probabilidades de atención, proyección, ramas MLP y residuales de cada bloque, normalización final y logits. Son oráculos de prueba externos. El constructor del modelo recibe únicamente checkpoint, IDs, constantes y límites: nunca recibe activaciones o logits de referencia.

### 4. Plan prestado del programa, orden conservado y temporales reutilizables

Se añadirá `ExecutionPlan<'program>` con campos privados y una referencia inmutable al `Program` validado. Evita clonar su tabla y conserva una relación inequívoca con los diagnósticos. Su vida no supera la del programa; el programa continúa independiente de la arena.

Interfaz prevista:

- `plan(&Program, PlanLimits) -> Result<ExecutionPlan<'_>, PlanError>`; límites explícitos de nodos y posiciones numéricas.
- `run(&ExecutionPlan, max_nodes) -> Result<Vec<f64>, EvalError>`; mismo presupuesto lógico y errores numéricos que el evaluador original.
- Estadísticas de instrucciones, posiciones y payloads, independientes de `ProgramStats`.

El plan conserva una instrucción por fila, con tres índices `usize`: operandos izquierdo/derecho y destino. Su posición i identifica el nodo lógico i+1; no necesita guardar un mapa adicional de diagnóstico por instrucción. Las raíces tienen un mapa de posiciones que preserva orden y duplicados.

Algoritmo de planificación, iterativo y lineal en V+E+R:

1. Recorrer pares y calcular último uso por nodo; marcar todas las raíces como vivas hasta recoger resultados.
2. Reservar la posición 0 para One durante toda la ejecución.
3. Resolver las posiciones de ambos operandos antes de liberar nada.
4. Liberar cada operando no raíz cuyo último uso sea esta instrucción, una sola vez si ambos operandos coinciden; asignar el destino desde una lista determinista de posiciones libres o extenderla.
5. Emitir la instrucción con las posiciones ya resueltas. Los vectores de último uso y asignación se descartan al terminar la planificación.

El ejecutor carga ambos operandos en valores locales antes de escribir el destino. Compartirá el kernel comprobado de `src/eval.rs` mediante visibilidad interna, preservando el orden de comprobaciones y las llamadas `exp`/`ln`. No hay HashMap, búsqueda de bloques ni reconocimiento algebraico en el bucle numérico. Cada llamada crea su propio buffer; no hay resultados guardados en el plan.

Se conserva el orden topológico actual en vez de reordenar operaciones para localidad: así se mantiene también el primer error observable. Se eligen `usize` y un plan simple antes de introducir índices compactos. Las instrucciones de tres índices añaden almacenamiento; junto con el programa pueden costar más que el ahorro de valores. Se reportarán ambos componentes, capacidad, salidas y metadatos, además del RSS. La primera mejora exigible es la reducción verificable del buffer, no del total.

Fixtures decisivas: One solo; raíces repetidas; una raíz usada también como operando; operandos idénticos; diamante compartido; último uso de ambos hijos en una instrucción; cadena estable `ti=Magma(One,ti-1)` de 10000 pasos; fallos tras reutilización; límites y ejecuciones intercaladas. La cadena exige dos posiciones, contando One.

### 5. Síntesis ampliada con una puerta de viabilidad real

Se conservarán los constructores y tolerancias originales. `src/synth/scalar.rs` añadirá el constructor de bits F32; `src/synth/model.rs` alojará las expansiones necesarias para el modelo. Ambos se exportan desde el módulo `synth` actual, sin crear una segunda representación ejecutable.

La estrategia candidata para constantes es decodificar exactamente el racional diádico F32 y construir mantisa/signo/escala usando One/Magma. Una mantisa mediante Horner fraccionario mantiene intermedios algebraicos acotados; duplicaciones o mitades componen la escala. Esto describe una familia de construcciones que debe derivarse y verificarse: no afirma que las expansiones actuales sean estables para esos pasos. La base de Magma sigue siendo `exp/ln`; el exponente del formato IEEE es metadata, no una operación `exp2` añadida.

Para sumas, productos con signo, recíprocos positivos, raíz inversa, exponencial y SiLU se derivarán identidades y dominios de las expansiones elegidas. Las reducciones tendrán agrupación fija documentada. No se introducen casts que evalúen expresiones, una búsqueda por resultados numéricos en construcción ni callbacks a los oráculos. Una identidad real y sus condiciones se documentan separadamente de la evidencia `f64`.

La comprobación temprana cubre:

1. Todos los bits distintos de pesos y constantes requeridos por las dos fixtures, con trazabilidad al tensor/índice.
2. Un corpus determinista de operandos F32 de las trazas: primeros elementos, mínimos/máximos, menor magnitud no nula y cancelaciones, de cada bloque y familia de operación. Se persiste antes de evaluar EML; los oráculos escalares calculan en F64 esos operandos F32.
3. Productos escalares completos representativos de longitudes 8, 64 y MLP, y fórmulas escalares/vectoriales de RMSNorm, softmax y SiLU. No basta probar solo un producto aislado.
4. Nodos, filas, profundidad, posiciones temporales y coste medido de esos programas. Se estima el crecimiento de las formas completas y se documentan supuestos, distinguiendo la estimación de la memoria medida.

Los límites de carga, construcción, compilación, planificación y evaluación se proporcionan explícitamente. El coste estimado se contrasta con esos límites antes de pedir la expansión completa. Si el crecimiento o la aritmética no caben, se detiene la integración dependiente con el caso mínimo y las tareas restantes pendientes. Pasar la puerta habilita las siguientes comprobaciones; no demuestra de antemano los cinco bloques ni permite relajar su criterio.

Es un riesgo técnico conocido y una tarea de viabilidad acotada, no una decisión de lenguaje pendiente. Si resolverlo exige entradas enlazadas, primitivas nativas, precisión distinta, cuantización o segmentar capas reinyectando valores, hace falta revisar el cambio con el usuario.

### 6. Construcción tensorial cerrada y atención sin infinitos

`src/tensor.rs` contendrá formas y referencias `NodeId` en orden row-major, con validación de dimensiones y productos de tamaños comprobados. Sus operaciones solo construyen estructura: selección de embedding, vistas/reshape, proyecciones, reducciones, productos elemento a elemento y residuales. Los pesos repetidos se sintetizan una vez por bits dentro de la misma arena, después de validar procedencia; ese caché contiene handles, nunca resultados de evaluación.

`src/models/llama.rs` implementará un constructor de bloque que se invoca cinco veces con sus pesos propios. La configuración se valida contra las formas del paquete. No se añade la normalización Q/K específica de Qwen.

- RMSNorm construye media de cuadrados, suma del epsilon fijado, raíz inversa y escala.
- RoPE usa los coeficientes posicionales importados y sintetizados, con la disposición por pares del original. No hay `sin`/`cos` numéricos dentro de EML.
- La atención construye solo términos j<=t, con dos cabezas Q por cabeza KV. No materializa una máscara `-inf`.
- Softmax usa diferencias respecto de un pivote simbólico de la fila (el primer score permitido), exponenciales y normalización positiva. Restar un pivote común conserva la fórmula real y evita necesitar `max` nativo; su rango y estabilidad deben pasar las mismas pruebas. No garantiza estabilidad para scores arbitrarios.
- SwiGLU construye `down(SiLU(gate(x)) * up(x))` y el residual correspondiente.

Se construye un único DAG cerrado por prefijo. Evaluar una capa y volver a introducir sus valores para construir la siguiente rompería el contrato elegido y no es una optimización permitida. El programa temporizado tiene solo los 512 logits finales como raíces. La variante diagnóstica añade raíces de etapas, registra su propio orden/identidad y se valida por separado, porque esas raíces cambian alcance y liveness.

### 7. Criterios numéricos fijos y verificables

Las nuevas tolerancias son decisiones iniciales de aceptación, no resultados medidos ni cotas matemáticas globales. No se ajustan después de ver fallos durante apply.

| Nivel | Referencia | Criterio por elemento |
| --- | --- | --- |
| Constantes importadas | Valor exacto del F32 elevado a F64 | `abs(error) <= 1e-9 + 1e-7*abs(ref)` |
| Corpus escalar y componentes aislados | Fórmulas F64 sobre los mismos operandos | `abs(error) <= 1e-6 + 1e-5*abs(ref)` |
| Etapas del modelo completo | Referencia explícita F64 | `abs(error) <= 1e-4 + 1e-4*abs(ref)` |
| 512 logits finales, todos los modos | PyTorch original F32 | `abs(error) <= 1e-4 + 1e-4*abs(ref)` |
| EML planificado frente a original | Mismo programa y entorno | `abs(error) <= 1e-12 + 1e-12*abs(ref)` |

La última tolerancia se verifica en fixtures finitas de equivalencia y también en ambos programas completos; no es una promesa portable de identidad bit a bit de funciones trascendentes. Se mantienen por separado los límites originales absolutos 1e-9, 1e-8 y 1e-6. NaN/infinito, error de dominio o desbordamiento invalidan el caso; no se excluyen al calcular métricas. Top-1, RMS y margen entre primeros logits son diagnósticos adicionales; un top-1 igual no sustituye la comparación de 512 valores.

### 8. Adaptadores y protocolo de benchmark

Los adaptadores externos viven en `tools/adapters/`; no se enlazan a la biblioteca. Reciben los IDs y posiciones fijados y devuelven logits, medidas y metadata mediante un protocolo versionado. Las revisiones exactas de PyTorch, llama2.c, llama.cpp, convertidor y compiladores se fijan antes de la primera comparación. La revisión llama2.c inspeccionada es un punto de partida, no evidencia de un binario ya verificado.

- PyTorch: modelo original F32 como oráculo y modo temporizado separado de la extracción de trazas.
- llama2.c: wrapper del `forward` real con pesos F32 y estado nuevo por repetición; recorre los IDs en orden. Se declara que su ruta original proyecta logits en cada posición, aunque solo se exporten los últimos.
- llama.cpp: conversión F32 comprobada, GPU desactivada, un hilo y contexto explícito; adaptador de la API de la revisión fijada que inyecta IDs y recupera los logits finales. No se usa un benchmark con tokens sintéticos como reemplazo de estas entradas.
- EML: modos original y planificado sobre programas equivalentes. Ambos incluyen en el tiempo de ejecución su buffer local y extracción de resultados; la planificación se informa aparte.

Cada modo/fixture corre en un proceso identificado, con tres calentamientos y diez repeticiones medidas como mínimo. Antes de cada repetición se vacían activaciones y estado KV del motor; los pesos y estructuras pueden permanecer cargados. El reinicio de estado se informa como preparación, con la misma política en todas las muestras del modo. No se retiene caché de prefijos ni logits previos. Se publican muestras, mediana y p95 mediante rango `ceil(0.95*n)`; para diez muestras el p95 es la máxima.

`tools/benchmark.py` registra carga, preparación, construcción, compilación, planificación y ejecución por separado, además del recorrido de extremo a extremo y sus exclusiones. Descargas e instalación no forman parte de inferencia. El tiempo de ejecutar repetidamente un DAG preparado no representa el de un prompt nuevo: cada nuevo prefijo exige construir otro programa.

Se usa medición monotónica del motor para su ejecución y reloj del proceso padre para extremo a extremo. El RSS máximo se obtiene del proceso hijo con el método y unidades de la plataforma registrados. Un high-water mark que incluye construcción se etiqueta como tal; no se deduce un pico por fase restando máximos. Los payloads EML se calculan con tamaños reales, incluyendo `Program`, plan, raíces, valores y capacidades. Ausencia de una métrica se registra como `unavailable` con motivo.

Resultado: JSON versionado con manifiesto, comandos, versiones, hashes, fases, muestras, memoria, comparación completa y estado por fixture/modo; resumen Markdown generado desde ese JSON. Un motor ausente, timeout, falta de memoria, conversión no equivalente o divergencia deja el resultado incompleto/inválido. No hay objetivo obligatorio de ganar en velocidad; sí de presentar una comparación válida y reproducible.

### 9. Organización de código y pruebas

```text
src/
  expr.rs                 Arena, pares e internado existentes
  synth.rs                API original y exportación de constructores nuevos
  synth/
    scalar.rs             Metadatos F32 y síntesis de constantes
    model.rs              Expansiones con dominios documentados
  program.rs              Snapshot lógico y compilación existentes
  eval.rs                 Evaluador original y kernel numérico compartido
  execution/
    mod.rs                API pública de planificación/ejecución
    plan.rs               Últimos usos, posiciones, límites y estadísticas
    run.rs                Recorrido secuencial del plan
  model_io.rs             Lector del paquete canónico validado
  tensor.rs               Formas y constructores de tensores de NodeId
  models/
    mod.rs
    llama.rs              Bloque y ensamblado stories260K
tools/
  import_model.py
  reference.py
  benchmark.py
  adapters/               Wrappers externos con revisiones fijadas
examples/
  infer_tiny.rs
tests/
  execution.rs
  model_io.rs
  model_synthesis.rs
  tensor.rs
  tiny_model.rs
```

Las pruebas ordinarias usan fixtures pequeñas locales; las verificaciones del checkpoint se ejecutan explícitamente después de preparar sus archivos, sin descargas ocultas durante `cargo test`. El reporte distingue la suite local de las pruebas completas del modelo. Las pruebas de integración pueden marcarse como externas, pero el cierre del cambio exige ejecutarlas y pasar ambos prefijos en todos los modos. Las anotaciones de omisión no satisfacen sus tareas.

## Risks / Trade-offs

- [Las expansiones pueden fallar con pesos/activaciones modestos] -> Puerta numérica temprana, identidades y dominios explícitos, evidencia de fallo y pausa de tareas dependientes; no se garantiza éxito del modelo por ser pequeño.
- [El DAG cerrado puede superar recursos antes de evaluar] -> Medir construcciones representativas, estimar crecimiento con formas reales y exigir límites explícitos; la reutilización de valores no elimina internado ni coste de compilación.
- [El plan añade más bytes de los que ahorra] -> Comparar payload total, capacidad y RSS además de posiciones y tiempos. La métrica de éxito inicial del plan es el buffer de la cadena y la corrección.
- [RoPE, formatos o alias difieren entre conversiones] -> Verificar tensores y layout canónico, posiciones y tokenizer; comprobar logits antes de comparar rendimiento.
- [F32 y F64 producen diferencias legítimas] -> Mantener dos referencias y thresholds por etapa, registrar precisión y preservar todas las divergencias.
- [Las raíces diagnósticas alteran el plan de memoria] -> Programas de validación y temporización identificados por separado; ambos pasan sus verificaciones.
- [Un checkpoint fijo sobreespecializa el resultado] -> Etiquetar el alcance exacto de dos prefijos; cualquier afirmación de generalidad requiere evidencia adicional.

## Migration Plan

1. Verificar la base completada y fijar datos, herramientas y referencias. Mantener todas las APIs existentes.
2. Añadir el plan opt-in y demostrar equivalencia/ahorro de posiciones sobre fixtures existentes y casos de liveness.
3. Añadir la nueva frontera de escalares y expansiones. Superar la puerta numérica y de recursos antes de ampliar a tensores y modelo.
4. Verificar componentes, un bloque y luego los cinco, con ambos prefijos y evaluadores.
5. Integrar adaptadores, verificar conversiones y publicar la comparación y su receta.
6. Ejecutar controles finales y solicitar revisión de la PR antes de merge/archivo, según el flujo de tareas.

La reversión consiste en dejar de usar el plan y las nuevas APIs de modelo y continuar con `evaluate` y la red original. El programa validado conserva su formato lógico; no hay migración de archivos de usuario ni persistencia nueva obligatoria. Los paquetes de herramientas llevan versión y no se reinterpretan silenciosamente.

## Fuentes consultadas

- [Checkpoint y configuración de entrenamiento fijados](https://huggingface.co/karpathy/tinyllamas/blob/0bd21da7698eaf29a0d7de3992de8a46ef624add/stories260K/readme.md).
- [Modelo PyTorch de referencia inspeccionado](https://github.com/karpathy/llama2.c/blob/350e04fe35433e6d2941dce5a1f53308f87058eb/model.py).
- [Runtime C inspeccionado](https://github.com/karpathy/llama2.c/blob/350e04fe35433e6d2941dce5a1f53308f87058eb/run.c).
- [Candidato GGUF, equivalencia aún pendiente](https://huggingface.co/ggml-org/test-model-stories260K/tree/479896ec924af6d40fd419ab8f4d1eb2101de00d).
- [API y ejemplo de inferencia de llama.cpp](https://github.com/ggml-org/llama.cpp/tree/master/examples/simple), consultados mediante documentación actual; la implementación fijará la revisión y usará su API real.
