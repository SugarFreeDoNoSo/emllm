# Spec Delta

## Purpose

Medir reproduciblemente la composición simbólica nativa y el efecto del empaquetado, identificando sus costes diferidos y su relación con controles de inferencia numérica.

## ADDED Requirements

### Requirement: Frontera temporal de composicion nativa
El tiempo nativo SHALL comenzar antes de validar la solicitud y crear su estado mutable, e incluir selección de embeddings, asociación de puertos, instanciación, internado y materialización de las 512 expresiones de salida. SHALL terminar cuando exista una salida cerrada utilizable, con los temporales internos de la llamada liberados.

#### Scenario: Medicion de una solicitud nueva
- **WHEN** se mide un prefijo con modelo y símbolos ya preparados
- **THEN** el intervalo contiene todo el trabajo estructural específico de esa solicitud, y no solo la recuperación de handles de una salida anterior

### Requirement: Fases excluidas visibles
Carga, preparación del modelo, codificación, compilación para decodificar, planificación numérica, evaluación, selección de token, comparación y emisión SHALL informarse separadas del tiempo nativo. SHALL publicarse también el extremo a extremo con alcance declarado. Excluir una fase del cronómetro principal SHALL NOT ocultar su trabajo o memoria.

#### Scenario: Evaluacion como verificacion externa
- **WHEN** se ejecuta exp/ln para leer los valores de una salida EML y compararlos
- **THEN** ese tiempo queda fuera de la composición y se registra como evaluación numérica de validación, incluyendo sus fallos y recursos

### Requirement: Semantica de resultados comparados
Los modos nativos SHALL etiquetarse como composición simbólica con salida de expresiones. Los controles externos SHALL etiquetarse como evaluación numérica. El reporte SHALL NOT presentar su cociente directo como aceleración de una misma inferencia numérica ni como latencia de generación de tokens. Los cocientes entre disposiciones nativas SHALL usar la misma frontera.

#### Scenario: Composicion mas rapida que un control numerico
- **WHEN** una muestra nativa termina antes que un motor que calcula los logits numéricos
- **THEN** se muestran ambos tiempos con sus tipos de salida y trabajo diferido, sin concluir una aceleración numérica equivalente

### Requirement: Conjunto de entrada y motores identificados
La aceptación SHALL incluir modos nativos no empaquetado y empaquetado, PyTorch, llama2.c y llama.cpp, con el checkpoint y prefijos originales de 8/16 IDs. Los controles SHALL usar CPU, un hilo y pesos F32 sin cuantización; se registrarán revisiones inmutables, compiladores, flags, hardware y precisiones efectivas.

#### Scenario: Matriz de resultados completa
- **WHEN** se declara completado el benchmark
- **THEN** existen resultados válidos para las diez combinaciones de cinco modos y dos prefijos, con identidades reproducibles y conversiones de pesos verificadas

### Requirement: Repeticiones con estado independiente
Cada modo y fixture SHALL realizar al menos tres calentamientos y diez repeticiones medidas. Cada llamada nativa SHALL componer con estado nuevo; los controles numéricos SHALL recomputar desde activaciones y KV vacíos. Podrán permanecer preparados el modelo independiente de tokens y la codificación de entradas, pero no resultados del modelo para ese prefijo.

#### Scenario: Entrada repetida despues del calentamiento
- **WHEN** se inicia una repetición medida del mismo prefijo
- **THEN** no se reutiliza su grafo de salida, sus logits ni su contexto KV, y la política de reinicio se conserva en todas las muestras del modo

### Requirement: Validacion de las muestras
Los resultados medidos SHALL validarse fuera de su intervalo contra los 512 logits F32 con `1e-4 + 1e-4*abs(ref)`. Los nativos SHALL pasar también la decodificación diferencial y la equivalencia entre disposiciones. Los errores SHALL conservar la muestra como inválida, sin convertirla en evidencia de rendimiento correcto.

#### Scenario: Fallo numerico despues de una composicion exitosa
- **WHEN** la evaluación externa de una muestra nativa desborda o excede una tolerancia
- **THEN** se conservan sus tiempos y diagnóstico, pero la combinación modo-fixture no satisface la aceptación

### Requirement: Estadisticas y evidencia reproducibles
El reporte SHALL incluir todas las muestras, mediana, p95 por rango `ceil(0.95*n)`, hashes de datos y fuentes, fases, límites y comandos. SHALL producir datos estructurados y un resumen coherente, distinguir medición de estimación y registrar métricas ausentes como no disponibles con motivo.

#### Scenario: Diez repeticiones
- **WHEN** se resumen diez duraciones válidas
- **THEN** se conservan las diez y el p95 corresponde a la máxima, sin sustituir la dispersión por una única duración

### Requirement: Memoria con alcance explicito
La contabilidad SHALL incluir base, plantilla, índices de internado, remapeos, estado por solicitud, raíces y costes de decodificación. SHALL distinguir payload, capacidad, memoria compartida y RSS. Un pico global que incluya preparación o validación SHALL NOT etiquetarse como pico exclusivo del núcleo nativo.

#### Scenario: Base compartida y pico de validacion
- **WHEN** varias vistas comparten constantes y el máximo RSS ocurre al evaluar la salida
- **THEN** la base se cuenta una vez en el total residente compartido, sus vistas se declaran, y el pico se atribuye al alcance que realmente fue observado

### Requirement: Presupuesto experimental independiente de los controles
Cada corrida SHALL declarar límites finitos de nodos, posiciones numéricas, memoria y tiempo antes de iniciarse. Podrán superar los recursos de los controles sin invalidar un resultado correcto. Agotar un límite SHALL identificar fase y perfil, conservar evidencia parcial e impedir declarar completo el caso.

#### Scenario: Modelo correcto con mayor consumo
- **WHEN** un modo nativo pasa la validación dentro de su perfil declarado y consume más que los controles
- **THEN** se acepta la comparación y se publica la diferencia sin exigir una mejora de rendimiento

#### Scenario: Limite ampliado aun insuficiente
- **WHEN** se agota un límite durante preparación, composición o validación
- **THEN** se informa el bloqueo, sin bajar la cobertura del modelo ni cambiar tolerancias para completar la corrida

### Requirement: Aceptacion independiente del benchmark anterior
Completar este experimento SHALL requerir preparación reutilizable, ambas disposiciones, validación completa y todos los controles. SHALL NOT marcar por equivalencia las tareas pendientes del benchmark de evaluación numérica anterior ni reclasificar sus resultados bajo la nueva frontera temporal.

#### Scenario: Solo el experimento nativo esta completo
- **WHEN** se satisface este cambio mientras el benchmark anterior conserva tareas pendientes
- **THEN** el reporte identifica ambos estados por separado y conserva la interpretación original de sus evidencias
