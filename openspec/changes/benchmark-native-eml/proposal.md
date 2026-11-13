# Proposal

## Why

El experimento actual mide la evaluación numérica de un DAG especializado por prefijo; necesitamos investigar una frontera distinta: preparar el modelo en EML y medir la composición de entradas nativas en expresiones de salida, usando la evaluación numérica únicamente para validación externa. Una referencia funcional, aunque consuma más recursos, permitirá después evaluar mejoras de representación y una futura destilación con costes observables.

## What Changes

- Preparar `stories260K` como constantes One/Magma y plantillas estructurales parametrizadas por entradas, antes de cronometrar. Una preparación por longitud y disposición deberá servir para distintos prefijos de esa longitud, sin conocer sus tokens concretos.
- Introducir entradas nativas con procedencia y un compositor que entregue las **512 expresiones EML de logits**, en un grafo cerrado. La asociación de entradas con embeddings, instanciación, internado y materialización de resultados pertenecen al tramo cronometrado; la evaluación `f64`, selección de token y comparación externa quedan fuera.
- Reutilizar entre corridas la base inmutable, la plantilla y las entradas codificadas. Cada solicitud tendrá su propio estado estructural; no reutilizar grafos de salida, activaciones ni resultados numéricos de solicitudes anteriores.
- Añadir una disposición de canales emparejados: dos canales reales como componentes real e imaginaria. Una matriz real `n × m` se presenta como `ceil(n/2) × m` pares, conservando ambas componentes, dimensiones originales y semántica real. Comparar esta disposición con la no empaquetada sin presuponer menor memoria, menos nodos o menos trabajo.
- Verificar fuera del cronómetro todos los logits y etapas requeridas mediante los evaluadores reales existentes y las referencias F32/F64 fijadas. Mantener sus tolerancias y registrar también top-1; la coincidencia del token no sustituye la comprobación de los logits.
- Medir dos modos nativos y los controles numéricos PyTorch, llama2.c y llama.cpp. Informar preparación, codificación, composición, decodificación/validación, extremo a extremo y memoria con sus respectivas fronteras. La comparación entre modos nativos tendrá el mismo tipo de salida; el contraste con motores numéricos identificará expresamente el trabajo diferido.
- Establecer presupuestos experimentales ampliados y configurables por fase, independientes del consumo de los otros motores. Se exige completar y verificar el experimento, no ganar en recursos; un agotamiento sigue siendo un resultado incompleto.

Decisiones confirmadas para este cambio: salida de expresiones EML y empaquetado de canales reales. Quedan para otros cambios la selección nativa de tokens, generación incremental, destilación/entrenamiento, aritmética compleja trascendente, cuantización, Redis y persistencia portable del modelo preparado. La preparación de este hito produce objetos inmutables en memoria, reutilizables durante la vida del proceso.

## Capabilities

### New Capabilities

- `eml-prepared-models`: preparación de constantes, vocabulario nativo y plantillas del modelo independientes de los tokens concretos, con procedencia, límites y propiedad explícitos.
- `eml-symbolic-inference`: composición por solicitud de entradas nativas en salidas One/Magma cerradas, aislamiento y validación externa de su significado numérico.
- `eml-channel-packing`: representación emparejada de canales reales y operaciones equivalentes del Transformer, con dimensiones y contabilidad conservadas.
- `eml-native-benchmarks`: protocolo reproducible de composición simbólica, controles numéricos, presupuestos y publicación de costes por frontera.

### Modified Capabilities

Ninguna especificación principal: `openspec list --specs --json` devuelve un inventario vacío. Se añaden capacidades; se preservan los contratos implementados de expresiones, programas, evaluadores y planes. Este experimento no completa ni redefine las tareas pendientes de `benchmark-tiny-model-eml`.

## Impact

- Nuevos tipos y módulos previstos en `src/native/`, exportados desde `src/lib.rs`; integración interna con síntesis y compilación validada, sin convertir puertos de plantilla en nodos públicos del lenguaje cerrado.
- Reutilización del lector EMLTINY, checkpoint, tokenizer, prefijos y referencias de `tools/`; nuevos ejemplos, verificadores y adaptadores del benchmark nativo. La biblioteca Rust mantiene únicamente `std`.
- Pruebas de preparación independiente de entradas, procedencia, aislamiento, cierre del grafo, equivalencia del empaquetado y del modelo completo; documentación de fórmulas, fases y resultados.
- Base inspeccionada: `develop`, revisión `4e22ab502a46565966d0c0ce338f8cd9a22fb2ad`. La síntesis de escalares y componentes está verificada; faltan el Transformer completo, el modelo parametrizado y los adaptadores temporizados. Su implementación forma parte de este cambio y no se presupone disponible.
