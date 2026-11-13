# Spec Delta

## Purpose

Comparar EML con motores de inferencia externos sobre un modelo y entradas equivalentes, con evidencia reproducible de corrección, preparación, ejecución y consumo de memoria.

## ADDED Requirements

### Requirement: Comparacion de motores identificados
La comparación de aceptación SHALL incluir EML original, EML planificado, PyTorch, llama2.c y llama.cpp en CPU, con un hilo de cómputo por motor y sin cuantización. SHALL registrar versiones o revisiones, compilador, flags, hardware, sistema, dispositivo, hilos efectivos y precisión de pesos, cálculo y cachés.

#### Scenario: Registro completo de una medicion
- **WHEN** se produce el reporte de aceptación para ambas fixtures
- **THEN** aparecen los cinco modos, sus configuraciones reproducibles y la diferencia entre EML `f64` y las rutas externas F32

### Requirement: Equivalencia de datos y conversiones
Cada adaptador SHALL consumir los IDs, posiciones y pesos fijados sin retokenizar. Una conversión SHALL verificar tensores, alias, parámetros de normalización, disposición RoPE y correspondencia del vocabulario con el origen. La coincidencia de nombre o tamaño del archivo SHALL NOT acreditar equivalencia.

#### Scenario: Variante GGUF publicada
- **WHEN** se propone usar un GGUF ya disponible
- **THEN** se exige la comprobación de equivalencia con el checkpoint original antes de incluir sus resultados como comparables

#### Scenario: Mapeo de pesos o tokenizer diferente
- **WHEN** el adaptador encuentra un tensor distinto, una permutación no justificada o IDs incompatibles
- **THEN** se informa la incompatibilidad y esa corrida no cuenta como comparación válida

### Requirement: Correccion previa a conclusiones de rendimiento
Cada modo SHALL devolver 512 logits finitos por fixture y cumplir `abs(actual-referencia_F32) <= 1e-4 + 1e-4*abs(referencia_F32)` para todos ellos. El reporte SHALL incluir error máximo, RMS, comparación elemento a elemento y top-1. Una divergencia SHALL invalidar conclusiones comparativas de rendimiento para esa corrida.

#### Scenario: Motor rapido con logits divergentes
- **WHEN** un motor termina antes que otro pero incumple la tolerancia en un logit
- **THEN** se conserva su tiempo como dato de una corrida inválida y no se presenta como una aceleración válida del mismo cálculo

### Requirement: Separacion de costes y trabajo realizado
La medición SHALL distinguir carga, preparación de entradas, construcción EML, compilación, planificación y ejecución. SHALL informar además el tiempo de extremo a extremo del recorrido medido y qué costes excluye. La ejecución de un grafo cerrado preparado SHALL NOT presentarse como tiempo para un prompt nuevo ni como generación incremental.

#### Scenario: Reutilizacion de un programa preparado
- **WHEN** se repite la ejecución EML sobre el mismo programa
- **THEN** se etiqueta como ejecución del programa preparado y el coste de construirlo sigue visible por separado

#### Scenario: Motor con recorrido secuencial
- **WHEN** un adaptador procesa el prefijo token por token y otro por lotes
- **THEN** ambos reciben los mismos IDs y se declara el recorrido, las proyecciones calculadas y el uso interno de caché en el reporte

### Requirement: Repeticiones sin reutilizacion de resultados
La medición SHALL realizar al menos tres calentamientos y diez repeticiones medidas por modo y fixture, registrando todas las duraciones, mediana y p95 por rango más próximo. Cada repetición SHALL recomputar la inferencia desde contexto numérico vacío; solo los pesos y estructuras de ejecución podrán permanecer preparados.

#### Scenario: Segunda repeticion con contexto previo
- **WHEN** se prepara una nueva repetición después de haber ejecutado la misma entrada
- **THEN** se descartan activaciones y estado de atención previo y se vuelven a calcular los logits, sin caché de resultados o prefijos

### Requirement: Memoria medida con alcance declarado
El reporte SHALL separar payloads estructurales, capacidad reservada, buffer numérico y pico de memoria residente del proceso. Cada medición SHALL identificar método, unidad y fases incluidas. Un dato no disponible SHALL aparecer como tal, nunca como cero ni como una estimación presentada como medición.

#### Scenario: Pico del proceso incluye construccion
- **WHEN** el mecanismo disponible informa un máximo que incluye importación y construcción
- **THEN** se etiqueta como pico del proceso completo y no como pico exclusivo de evaluación

### Requirement: Evidencia utilizable y fallos explicitos
La herramienta SHALL producir resultados estructurados y una síntesis legible, con manifiesto de inputs, comandos, revisiones, muestras y errores. Una dependencia ausente, fallo numérico, timeout o límite agotado SHALL quedar registrado por etapa. El benchmark completo SHALL requerir ambos prefijos y todos los modos válidos.

#### Scenario: Motor ausente o tarea incompleta
- **WHEN** no puede ejecutarse uno de los motores o una fixture
- **THEN** el reporte identifica la causa y conserva resultados parciales como parciales, sin declarar completado el benchmark
