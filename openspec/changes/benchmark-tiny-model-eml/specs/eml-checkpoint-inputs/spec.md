# Spec Delta

## Purpose

Introducir pesos y entradas de un checkpoint entrenado en EML mediante datos reproducibles, validación explícita y síntesis cerrada, preservando la procedencia y la precisión de referencia.

## ADDED Requirements

### Requirement: Identidad reproducible del conjunto de entrada
La importación SHALL registrar repositorio, revisión inmutable y SHA-256 de los archivos de origen, configuración, tokenizer, pesos, constantes auxiliares y fixtures. Las transformaciones SHALL declarar versión, orden de tensores, tipos, layouts y alias. Una identidad incompleta SHALL impedir declarar reproducible una comparación.

#### Scenario: Exportacion repetida
- **WHEN** se importa dos veces la misma revisión con la misma configuración de herramientas
- **THEN** los datos canónicos, token IDs y hashes de contenido coinciden, excluyendo únicamente metadatos ambientales identificados como tales

#### Scenario: Archivo de origen cambiado
- **WHEN** el contenido descargado no coincide con el SHA-256 fijado
- **THEN** se rechaza la preparación antes de construir un programa EML

### Requirement: Fidelidad de pesos y constantes
La representación importada SHALL preservar los bits F32 de los pesos originales y sus alias, sin cuantización ni sustitución por racionales del dominio inicial. Las constantes posicionales y de configuración SHALL tener procedencia y precisión declaradas. Todas SHALL convertirse a expresiones One/Magma antes de evaluar el modelo.

#### Scenario: Embedding compartido con salida
- **WHEN** el checkpoint declara la misma matriz para embedding y proyección de salida
- **THEN** la importación conserva esa identidad y ambos usos referencian los mismos pesos canónicos

#### Scenario: Coeficientes posicionales preparados externamente
- **WHEN** se importan senos y cosenos para las posiciones de las fixtures
- **THEN** sus bits y método de preparación quedan registrados y se sintetizan como constantes, sin introducir operaciones trigonométricas ejecutables en EML

### Requirement: Validacion del formato antes de construir
La carga SHALL comprobar versión, dimensiones, longitudes, rangos, alias y finitud antes de crear nodos. SHALL usar comprobaciones de desbordamiento para tamaños y offsets y límites explícitos de carga. Un fallo SHALL identificar su categoría y campo o tensor, sin entregar un modelo parcial válido.

#### Scenario: Datos malformados o incompatibles
- **WHEN** el archivo está truncado, tiene versión desconocida, tamaños desbordados, tensor ausente, alias inválido, dimensión incompatible o un escalar NaN/infinito
- **THEN** se rechaza la carga con una categoría identificable antes de construir el DAG

#### Scenario: Carga por encima del limite
- **WHEN** el tamaño declarado o real excede el límite explícito de carga
- **THEN** se rechaza antes de reservar el almacenamiento correspondiente

### Requirement: Sintesis verificable de los escalares requeridos
La síntesis SHALL representar mediante One/Magma todos los valores finitos distintos requeridos por el checkpoint y las fixtures fijadas. Su error absoluto SHALL ser como máximo `1e-9 + 1e-7*abs(valor_F32)`. SHALL documentar la identidad real y el dominio de cada construcción, sin evaluar subexpresiones durante su construcción.

#### Scenario: Pesos y constantes del corpus completo
- **WHEN** se sintetizan y evalúan todos los valores distintos de los pesos, escalas, epsilon y coeficientes posicionales fijados
- **THEN** todos cumplen la tolerancia, sus nodos son exclusivamente One/Magma y se registra el coste de síntesis

#### Scenario: Valor que aun no puede sintetizarse
- **WHEN** un valor requerido excede el dominio demostrado de una construcción o no cumple la tolerancia al verificarse
- **THEN** se informa el valor por bits y su ubicación y el corpus no se declara soportado

### Requirement: Entradas fijas previas a la evaluacion EML
El conjunto de aceptación SHALL contener dos prefijos de 8 y 16 token IDs del tokenizer original tok512, con política BOS/EOS explícita y posiciones desde cero. SHALL fijarse antes de medir EML. La ejecución SHALL consumir esos IDs sin retokenizar, añadir tokens ni sustituir entradas para conseguir una comparación exitosa.

#### Scenario: Mismos prefijos entre motores
- **WHEN** se preparan las dos fixtures para todos los motores
- **THEN** sus listas de IDs, longitudes, posiciones y hashes coinciden exactamente

### Requirement: Compatibilidad de la sintesis original
La ampliación SHALL conservar el dominio y los errores del constructor racional existente, así como los presupuestos, procedencia y tolerancias originales: 1e-9 para racionales, 1e-8 para operaciones escalares y 1e-6 para la red 2-2-1.

#### Scenario: Dominio anterior y nuevas entradas
- **WHEN** se ejecutan las pruebas originales junto con la síntesis de pesos F32
- **THEN** el constructor racional sigue rechazando 1/3 y las fixtures existentes conservan sus contratos y tolerancias

### Requirement: Puerta de viabilidad del checkpoint
La integración completa SHALL exigir evidencia satisfactoria de síntesis, operaciones y coste del corpus real. Un fallo de dominio, finitud, tolerancia o presupuesto SHALL bloquear la integración dependiente, conservar las tareas incumplidas y señalar el caso reproducible. SHALL NOT tratarse como ejecución exitosa del modelo.

#### Scenario: Fallo numerico durante la comprobacion inicial
- **WHEN** una operación con los datos fijados desborda, viola el dominio o excede su tolerancia
- **THEN** se registra la operación, sus entradas, categoría y nodo lógico si existe, y se requiere revisar el diseño antes de continuar con las tareas dependientes
