# Spec Delta

## Purpose

Preparar un modelo reutilizable en representación EML antes de recibir los tokens concretos de una solicitud, preservando pesos, procedencia y límites observables.

## ADDED Requirements

### Requirement: Identidad y fidelidad del modelo preparado
La preparación SHALL usar el checkpoint stories260K fijado, conservar sus bits F32 de origen y alias, y registrar hashes, configuración y versión de transformación. Los pesos y constantes ejecutables SHALL representarse mediante One/Magma antes de aceptar solicitudes nativas.

#### Scenario: Checkpoint y salida compartida
- **WHEN** se prepara el paquete canónico verificado
- **THEN** se conserva la configuración de cinco bloques, dimensión 64, MLP 172, cabezas Q/KV 8/4 y vocabulario 512, y embedding y proyección de salida comparten los mismos pesos nativos

#### Scenario: Procedencia incompatible
- **WHEN** el paquete tiene un hash, configuración, forma o alias incompatible con el conjunto fijado
- **THEN** la preparación se rechaza antes de entregar un modelo utilizable

### Requirement: Preparacion independiente del contenido de la entrada
La preparación SHALL depender del modelo, longitud, disposición y versión de síntesis, sin recibir tokens concretos, activaciones ni resultados de referencia. Una preparación para longitud 8 o 16 SHALL servir para distintos prefijos de esa misma longitud.

#### Scenario: Dos prefijos y una preparacion
- **WHEN** se codifican y componen dos prefijos diferentes de longitud 8 contra el mismo modelo preparado
- **THEN** se reutilizan exactamente la base y la plantilla preparadas, sin reconstruirlas ni modificar su identidad, y ambas salidas se validan contra sus referencias independientes

#### Scenario: Longitud incompatible
- **WHEN** una solicitud tiene una longitud distinta de la declarada por la preparación
- **THEN** se devuelve un error de forma antes de componer sus operaciones

### Requirement: Preparacion exclusivamente estructural
La transformación a EML SHALL construir estructura sin evaluar sus nodos ni consultar oráculos. Las representaciones de las operaciones SHALL estar preparadas antes de iniciar la composición cronometrada. Un éxito estructural SHALL NOT declararse prueba de corrección numérica.

#### Scenario: Plantilla con fallo numerico futuro
- **WHEN** se prepara una plantilla pequeña estructuralmente válida cuya instanciación tendrá un error de dominio al evaluarse
- **THEN** la preparación y composición pueden finalizar, y el fallo se detecta durante la validación numérica externa

### Requirement: Puertos confinados a plantillas
Las entradas formales SHALL existir únicamente en la representación de plantillas. Una plantilla con puertos SHALL NOT ser aceptada como programa cerrado evaluable. Sus referencias deberán tener procedencia válida y dependencias existentes, sin ciclos ni índices arbitrarios fabricables por la API segura.

#### Scenario: Plantilla sin asociar
- **WHEN** un consumidor intenta evaluar una plantilla que todavía contiene puertos de entrada
- **THEN** la API impide tratarla como un programa cerrado

#### Scenario: Referencia ajena a la plantilla
- **WHEN** se intenta construir una operación con una referencia perteneciente a otro contexto de preparación
- **THEN** se rechaza por procedencia sin publicar una plantilla parcial como válida

### Requirement: Codificacion nativa limitada a simbolos de entrada
La codificación SHALL transformar los IDs y posiciones fijados en símbolos nativos del vocabulario preparado, preservando su orden y procedencia. SHALL NOT seleccionar embeddings, calcular activaciones, asociar puertos del modelo ni construir resultados dependientes de esos tokens; ese trabajo pertenece a la composición.

#### Scenario: Codificacion de un prefijo
- **WHEN** se codifican los IDs de una fixture válida
- **THEN** se obtiene la misma secuencia de símbolos y posiciones, sin ejecutar operaciones del modelo ni aumentar su grafo de constantes

#### Scenario: Simbolo fuera del vocabulario
- **WHEN** aparece un ID igual a 512 o un símbolo de otro modelo preparado
- **THEN** se rechaza la entrada con un error de rango o procedencia, respectivamente

### Requirement: Propiedad e inmutabilidad de la preparacion
El modelo preparado SHALL conservar su estructura tras liberar los datos de importación y los constructores temporales. La composición, validación o fallo de una solicitud SHALL NOT modificar sus constantes, plantilla, alias ni identidad.

#### Scenario: Fuentes liberadas y solicitudes intercaladas
- **WHEN** se liberan el paquete y los constructores y se intercalan solicitudes válidas y fallidas
- **THEN** la preparación sigue siendo utilizable y sus inspecciones coinciden con las anteriores a esas solicitudes

### Requirement: Presupuestos explicitos de preparacion
La preparación SHALL recibir límites finitos, comprobar tamaños antes de reservar y distinguir límites de constantes, plantillas, puertos y raíces. Los costes y fallos SHALL identificar la fase y el presupuesto usado, sin entregar modelos incompletos como utilizables.

#### Scenario: Limite de constantes agotado
- **WHEN** la síntesis compartida alcanza el límite configurado antes de terminar los pesos
- **THEN** se informa el tensor, índice o patrón correspondiente y la plantilla dependiente permanece sin preparar

### Requirement: Compatibilidad del nucleo cerrado existente
La ampliación SHALL preservar la procedencia, semántica real exp/ln, errores y tolerancias de las APIs cerradas existentes. La introducción de plantillas y símbolos nativos SHALL NOT habilitar hojas numéricas ni puertos en los programas que aceptan los evaluadores existentes.

#### Scenario: Contratos anteriores junto a plantillas
- **WHEN** se ejecutan los contratos originales y los nuevos casos de composición
- **THEN** continúan pasando las comprobaciones de referencias ajenas, snapshots, dominio, límites y precisión originales
