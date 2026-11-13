# Proposal

## Why

EML ya representa y evalúa DAGs One/Magma, pero todavía no puede ejecutar los pesos de un modelo entrenado ni comparar su lectura y memoria con motores externos. `stories260K`, con cinco bloques y vocabulario de 512 tokens, permite plantear esa comparación con un alcance menor que Qwen y comprobar primero los obstáculos numéricos reales.

## What Changes

- Incorporar una entrada reproducible del checkpoint original `karpathy/tinyllamas/stories260K`, sus constantes posicionales y dos secuencias de 8 y 16 token IDs fijadas antes de medir EML.
- Añadir síntesis de los escalares F32 del checkpoint a expresiones cerradas One/Magma y constructores numéricos aptos para las operaciones del modelo. Mantener `Magma(a,b)=exp(a)-ln(b)` y evaluación `f64`.
- Introducir un plan de ejecución derivado de `Program`, con reutilización de temporales por último uso, recorrido secuencial y diagnósticos en índices lógicos. Conservar el evaluador actual como referencia.
- Construir embedding, RMSNorm, proyecciones, RoPE, atención causal agrupada, SwiGLU y residuales; ensamblar los cinco bloques y obtener los 512 logits de la última posición.
- Comparar EML de referencia y optimizado con PyTorch, llama2.c y llama.cpp en CPU, usando pesos equivalentes y las mismas entradas. Separar preparación, construcción, compilación, planificación, ejecución, memoria y error.
- Establecer una puerta de viabilidad: verificar constantes, operaciones con datos reales y coste estructural antes de expandir el modelo. Un fallo bloquea las tareas dependientes; no satisface el objetivo del modelo ni autoriza relajar tolerancias o sustituir aritmética EML.

El hito es prefill completo de un checkpoint fijo. Generación incremental/KV persistente, Qwen, destilación, cambio a `exp2/log2`, GPU/MLX, cuantización, índices compactos y plantillas ejecutables compartidas quedan fuera de este cambio.

## Capabilities

### New Capabilities

- `eml-execution-plans`: planificación y ejecución de programas validados con temporales reutilizables y costes observables.
- `eml-checkpoint-inputs`: importación reproducible, validación y síntesis EML de los datos del checkpoint y sus fixtures.
- `eml-tiny-model-inference`: construcción y validación del Transformer pequeño completo mediante One/Magma.
- `eml-engine-benchmarks`: comparación reproducible de logits, tiempos y memoria entre EML y motores externos.

### Modified Capabilities

Ninguna especificación principal: el inventario `openspec/specs/` está vacío. Los contratos implementados por `implement-one-magma-eml`, `formalize-rust-core-contracts` y `use-internal-pair-tables` son la base de compatibilidad; este cambio añade capacidades sin reescribir sus deltas.

## Impact

- Nuevos módulos previstos: `src/execution/`, `src/model_io.rs`, `src/tensor.rs` y `src/models/llama.rs`; extensiones de síntesis en `src/synth/` y reutilización interna del kernel de `src/eval.rs`.
- `Arena`, `Program`, inspección, presupuestos lógicos y APIs existentes conservan sus contratos y tolerancias. Las nuevas fronteras de entrada usan tipos adicionales; `Rational` mantiene su dominio actual.
- Herramientas externas en `tools/`, fixtures pequeñas, pruebas, `examples/infer_tiny.rs` y documentación de reproducción. Python/PyTorch y los motores C/C++ son dependencias de desarrollo; la biblioteca Rust sigue usando solo `std`.
- La ejecución completa sigue siendo un resultado por demostrar. Reducir el modelo o el buffer numérico no demuestra por sí mismo estabilidad, aceleración ni menor memoria total.
