# Spec Delta

## Purpose

Construir y ejecutar un Transformer entrenado pequeño como un programa EML cerrado, verificando sus etapas y los logits completos contra referencias numéricas independientes.

## ADDED Requirements

### Requirement: Configuracion exacta del modelo objetivo
La inferencia SHALL usar los cinco bloques de stories260K, dimensión 64, ocho cabezas Q, cuatro KV, dimensión por cabeza 8 y vocabulario 512. SHALL validar la dimensión MLP, epsilon, RoPE, contexto y pesos compartidos contra el checkpoint fijado, y rechazar configuraciones incompatibles en vez de reinterpretarlas.

#### Scenario: Modelo correcto y dimensiones incoherentes
- **WHEN** se carga el modelo fijado y una variante de prueba con formas o conteos de cabezas incoherentes
- **THEN** se acepta únicamente el primero y se identifica la incompatibilidad del segundo

### Requirement: Programa cerrado de aritmetica EML
Toda aritmética del modelo SHALL expandirse a One/Magma antes de evaluar: embedding seleccionado, proyecciones, reducciones, RMSNorm, RoPE, atención, SwiGLU y residuales. SHALL NOT usar hojas numéricas, entradas mutables, activaciones de referencia ni operaciones tensoriales nativas como sustitutos durante la evaluación.

#### Scenario: Inspeccion de logits e intermedios
- **WHEN** se inspecciona cualquier programa del modelo, incluido uno con salidas de diagnóstico
- **THEN** todos sus nodos son One/Magma, todas sus dependencias están cerradas y el evaluador solo necesita el programa y sus límites

### Requirement: Semantica de normalizacion y activacion
La construcción SHALL reproducir RMSNorm con el epsilon fijado, SiLU y el producto de puertas SwiGLU, sin sesgos inexistentes ni normalización adicional de Q/K. Las expansiones SHALL justificar sus identidades y dominios; fallos numéricos SHALL propagarse sin clamps o valores de reemplazo.

#### Scenario: Componentes comparados por separado
- **WHEN** se ejecutan los componentes sobre el corpus escalar y vectorial fijado de las trazas de referencia
- **THEN** sus resultados son finitos y cada elemento satisface `abs(actual-referencia_F64) <= 1e-6 + 1e-5*abs(referencia_F64)`

### Requirement: Atencion causal agrupada y RoPE
La atención SHALL respetar posiciones, rotaciones por pares y agrupación de dos cabezas Q por cabeza KV. La posición t SHALL depender solo de posiciones 0..t. La máscara causal SHALL excluir contribuciones futuras sin introducir constantes infinitas en el programa EML.

#### Scenario: Cambio de un token futuro
- **WHEN** se cambia un token posterior a t y se inspeccionan las activaciones hasta t de ambos programas
- **THEN** las activaciones anteriores conservan sus valores dentro de la tolerancia de etapa y la atención no incorpora dependencias de ese token futuro

#### Scenario: Cabezas KV compartidas
- **WHEN** se verifican las ocho cabezas de una fixture con proyecciones Q distintas
- **THEN** cada pareja Q usa la cabeza KV correspondiente y sus resultados satisfacen la tolerancia de etapa frente a la referencia

### Requirement: Validacion de etapas completas
La verificación SHALL ofrecer salidas identificadas por bloque, etapa, posición y coordenada para embedding, normalizaciones, Q/K/V, RoPE, atención, MLP y residuales. Cada elemento de las etapas verificadas SHALL satisfacer `abs(actual-referencia_F64) <= 1e-4 + 1e-4*abs(referencia_F64)`. Los fallos SHALL conservar su diagnóstico lógico.

#### Scenario: Primer bloque y sucesion de cinco bloques
- **WHEN** se verifican primero un bloque y luego las trazas de los cinco bloques para ambas fixtures
- **THEN** todas las etapas cumplen la tolerancia y un fallo identifica bloque, etapa y coordenada, además del nodo lógico cuando corresponda

### Requirement: Logits completos para ambos prefijos
La inferencia SHALL devolver los 512 logits de la última posición para cada prefijo de 8 y 16 tokens, después de los cinco bloques, normalización final y proyección de salida. Cada logit SHALL satisfacer `abs(actual-referencia_F32) <= 1e-4 + 1e-4*abs(referencia_F32)`. El cumplimiento SHALL comprobarse con ambos evaluadores EML.

#### Scenario: Inferencia completa del checkpoint
- **WHEN** se ejecutan ambas fixtures con pesos originales y presupuestos suficientes
- **THEN** cada evaluador devuelve 512 valores finitos por fixture, todos cumplen la tolerancia y se reportan también error máximo, error RMS y token de mayor logit

#### Scenario: Solo una capa o una salida parcial disponible
- **WHEN** únicamente pasa la validación de un bloque, una fixture o un subconjunto de logits
- **THEN** se registra ese avance pero la inferencia completa permanece incumplida

### Requirement: Rechazo explicito y propiedad del programa
Las entradas inválidas, formas incompatibles y presupuestos agotados SHALL devolver errores por etapa sin entregar un programa parcial válido. Un programa completado SHALL seguir siendo inspeccionable y ejecutable después de liberar datos de importación y arena. Los datos numéricos de una corrida SHALL NOT alimentar la construcción de otra capa.

#### Scenario: Error de entrada o construccion
- **WHEN** se entrega un token fuera de vocabulario, posiciones incompatibles o un presupuesto insuficiente
- **THEN** se rechaza la operación con su categoría y sin presentar logits incompletos como éxito

#### Scenario: Fuentes liberadas
- **WHEN** se construye un programa completo, se liberan el checkpoint importado y su arena y después se evalúa
- **THEN** conserva sus pares, raíces y estadísticas y produce logits dentro de la tolerancia requerida
