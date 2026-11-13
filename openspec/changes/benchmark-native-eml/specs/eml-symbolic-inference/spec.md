# Spec Delta

## Purpose

Componer un modelo preparado y una entrada nativa en expresiones EML cerradas, con estado por solicitud y una validación numérica externa explícitamente separada.

## ADDED Requirements

### Requirement: Salida nativa cerrada de logits
La composición SHALL devolver 512 expresiones lógicas One/Magma para los logits de la última posición, tras los cinco bloques del modelo. Todas sus dependencias SHALL estar resueltas en estructura propia o base inmutable retenida; SHALL NOT quedar puertos, callbacks, hojas numéricas ni referencias a un contexto liberado.

#### Scenario: Salida inspeccionable de ambas disposiciones
- **WHEN** termina una composición completa, no empaquetada o empaquetada
- **THEN** se inspeccionan 512 raíces lógicas en orden de vocabulario, aunque se agrupen en 256 pares, y toda su clausura contiene únicamente One/Magma

#### Scenario: Uso posterior de una salida
- **WHEN** se libera el contexto de preparación de la plantilla y se usa una raíz de salida como operando de otra composición nativa
- **THEN** sus dependencias siguen disponibles y la operación se construye sin decodificar valores numéricos

### Requirement: Trabajo por entrada dentro de la composicion
La composición SHALL incluir validación nativa de la solicitud, selección de embeddings, asociación de puertos, instanciación de operaciones, internado y materialización de raíces. SHALL NOT limitarse a devolver una salida preparada previamente para esos tokens.

#### Scenario: Prefijo nuevo con modelo preparado
- **WHEN** llega un prefijo no usado durante la preparación
- **THEN** se seleccionan sus embeddings y se construyen sus dependencias dentro de la llamada de composición, conservando la base preparada

### Requirement: Ausencia de evaluacion numerica en el camino nativo
Preparar, codificar y componer SHALL usar únicamente operaciones estructurales y metadatos para el cálculo del modelo. SHALL NOT ejecutar exp/ln numéricos, operadores tensoriales numéricos, selección por valores evaluados ni consultas al oráculo. La decodificación posterior SHALL ser una operación separada.

#### Scenario: Composicion con evaluador deshabilitado
- **WHEN** se construye y compone una fixture con la frontera de evaluación numérica instrumentada para fallar si se invoca
- **THEN** se obtiene la salida estructural sin invocar esa frontera, y solo la llamada explícita de validación intenta evaluar

### Requirement: Aislamiento y reutilizacion solo de estructura preparada
Cada solicitud SHALL tener identidad y estado estructural propios. Podrán compartirse la base y plantilla independientes de tokens y las entradas codificadas. SHALL NOT reutilizarse resultados del modelo, grafos específicos de prefijo ni el índice de internado mutable de una solicitud anterior.

#### Scenario: Misma entrada repetida
- **WHEN** se compone dos veces el mismo prefijo sobre la misma preparación
- **THEN** ambas llamadas realizan la instanciación con estado nuevo, producen estructuras equivalentes y no usan la primera salida como caché

#### Scenario: Fallo entre dos entradas validas
- **WHEN** una solicitud agota su presupuesto entre dos composiciones válidas
- **THEN** conserva un error propio, no entrega salida parcial como válida y no altera las otras solicitudes ni el modelo preparado

### Requirement: Rechazos estructurales con procedencia
La composición SHALL rechazar símbolos ajenos, formas incompatibles, dependencias inválidas y límites agotados con errores identificables por fase. Las referencias del modelo, plantilla y solicitud SHALL NOT ser intercambiables únicamente por coincidir sus índices enteros.

#### Scenario: Indices iguales de solicitudes diferentes
- **WHEN** se intenta usar un handle de otra solicitud que tiene el mismo índice local que un handle válido
- **THEN** se rechaza por procedencia, sin modificar la salida válida

### Requirement: Fidelidad numerica de etapas y logits
La validación externa SHALL comprobar etapas frente a la referencia explícita F64 y todos los logits frente al original F32 con `abs(error) <= 1e-4 + 1e-4*abs(ref)`. SHALL exigir valores finitos y conservar diagnóstico por bloque, etapa y coordenada. Top-1 SHALL ser un diagnóstico adicional.

#### Scenario: Prefijos fijados y prueba de reutilizacion
- **WHEN** se validan ambas disposiciones para los prefijos fijados de 8/16 tokens y una variante determinista adicional de cada longitud
- **THEN** se verifican todas las etapas requeridas y los 512 logits de cada caso contra referencias generadas antes de ejecutar la composición nativa

#### Scenario: Token coincidente con logit incorrecto
- **WHEN** coincide el token ganador pero algún logit excede su tolerancia
- **THEN** el caso se declara numéricamente inválido y no satisface la aceptación

### Requirement: Validacion diferencial y corpus conservado
La decodificación SHALL usar los evaluadores original y planificado sobre el mismo programa validado, con diferencia máxima `1e-12 + 1e-12*abs(ref)`. Se conservarán las tolerancias de constantes `1e-9 + 1e-7*abs(ref)` y componentes `1e-6 + 1e-5*abs(ref)`, así como las originales del núcleo.

#### Scenario: Grafo valido que falla al decodificar
- **WHEN** una salida estructuralmente válida produce dominio inválido, desbordamiento o un valor no finito durante la decodificación
- **THEN** se registra el fallo numérico externo y el caso no se presenta como inferencia validada, aunque la composición haya terminado

### Requirement: Diagnosticos separados de las salidas medidas
Las salidas diagnósticas SHALL identificar todas las etapas necesarias para localizar divergencias. Una variante con raíces intermedias SHALL declarar su propia identidad, alcance y costes; SHALL NOT sustituir silenciosamente al programa medido con solo los logits finales como raíces.

#### Scenario: Raices adicionales de RMSNorm y atencion
- **WHEN** se solicita una composición diagnóstica con esas raíces además de logits
- **THEN** sus medidas se identifican como diagnósticas y la muestra principal sigue correspondiendo a la variante de 512 logits
