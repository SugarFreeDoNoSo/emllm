# Proposal

## Why

Queremos implementar EML como lenguaje efectivo de una red, partiendo de su completitud y usando exclusivamente `One` y `Magma`. El repositorio todavía no contiene código: el prototipo descrito en conversación no está disponible localmente y conserva primitivas aritméticas que este cambio eliminará del programa ejecutable.

## What Changes

- Crear un núcleo Rust con expresiones `One` y `Magma(a,b)`, cuya semántica es `exp(a) - ln(b)` para `b > 0`.
- Construir DAGs cerrados completos antes de evaluarlos, con identidad estructural, validación y orden topológico.
- Incorporar constructores aritméticos que expandan a esas dos únicas formas, incluyendo representación de entradas, pesos y sesgos racionales acotados.
- Evaluar con errores explícitos de dominio, desbordamiento y límites de recursos; distinguir equivalencia matemática de precisión numérica.
- Demostrar una inferencia de red pequeña con activación `exp(-z*z)` y comparar contra una referencia aritmética independiente.
- Documentar las fronteras: no se incorpora todavía entrenamiento, destilación, Transformer, GGUF, codec de bytes ni el prototipo inaccesible. No se promete aceleración ni representación exacta de todos los reales.

## Capabilities

### New Capabilities

- `eml-expressions`: construcción, síntesis acotada, compilación y evaluación de expresiones exclusivamente One/Magma.
- `eml-network-inference`: construcción e inferencia de una red pequeña enteramente expresada en ese lenguaje.

### Modified Capabilities

Ninguna; no existen especificaciones previas.

## Impact

Se propone un proyecto Cargo nuevo con biblioteca, ejemplo ejecutable y pruebas nativas, usando inicialmente la biblioteca estándar. Los valores numéricos podrán existir en la interfaz de construcción y en el evaluador, pero nunca como hojas o instrucciones alternativas del programa EML. Cambiar una entrada o peso requerirá construir la expresión correspondiente; no habrá enlaces dinámicos `f64` en el DAG.
