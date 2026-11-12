# Spec Delta

## Purpose

Ofrecer evidencia reproducible sobre el tamaño de los programas One/Magma y la complejidad del código Rust, con alcances explícitos que permitan interpretar ambas medidas sin confundirlas con corrección numérica o rendimiento.

## ADDED Requirements

### Requirement: Estadisticas estructurales alcanzables
La inspección pública de un programa validado SHALL informar nodos únicos alcanzables, enlaces de operandos, posiciones de raíz y profundidad máxima en enlaces. Cada posición de operando SHALL contar como un enlace, aunque repita hijo; One SHALL tener profundidad cero. Las raíces repetidas SHALL contar como posiciones distintas sin duplicar nodos ni enlaces.

#### Scenario: Programa elemental
- **WHEN** se consultan las estadísticas de un programa con una única raíz One
- **THEN** se informan 1 nodo, 0 enlaces, 1 posición de raíz y profundidad máxima 0

#### Scenario: Operandos compartidos
- **WHEN** se consulta un programa con una única raíz Magma(One,One)
- **THEN** se informan 2 nodos únicos, 2 enlaces, 1 posición de raíz y profundidad máxima 1

#### Scenario: Subexpresion compartida en dos niveles
- **WHEN** se consulta un programa cuya única raíz es Magma(t,t), donde t es Magma(One,One)
- **THEN** se informan 3 nodos únicos, 4 enlaces, 1 posición de raíz y profundidad máxima 2

#### Scenario: Raices repetidas
- **WHEN** se consulta un programa con las raíces [t,t], donde t es Magma(One,One)
- **THEN** se informan 2 nodos únicos, 2 enlaces, 2 posiciones de raíz y profundidad máxima 1

### Requirement: Inspeccion estructural independiente de valores numericos
Obtener estadísticas SHALL NOT evaluar exp o ln sobre el programa. Los nodos no alcanzables desde las raíces seleccionadas SHALL quedar fuera de las estadísticas. Para un programa dado, consultas repetidas SHALL devolver los mismos valores incluso después de una evaluación fallida o de ampliar su arena de origen.

#### Scenario: Programa con fallo numerico
- **WHEN** se consultan las estadísticas de un programa bien formado cuya evaluación provoca un error de dominio o desbordamiento
- **THEN** la consulta estructural tiene éxito y devuelve los mismos valores antes y después de esa evaluación

#### Scenario: Expresiones no utilizadas
- **WHEN** una arena contiene expresiones adicionales que no son alcanzables desde las raíces de un programa compilado
- **THEN** esas expresiones no contribuyen a ninguna de sus estadísticas

### Requirement: Reporte ciclomatico reproducible
El procedimiento de análisis del código SHALL producir resultados identificables por archivo y unidad analizada, acompañados de herramienta y versión o revisión exacta, comando, alcance, revisión del código y modificaciones locales. SHALL conservar la salida original e indicar cómo trata métodos, cierres, pruebas y código generado o expandido por macros.

#### Scenario: Medicion del nucleo
- **WHEN** se realiza el análisis ciclomático del núcleo Rust
- **THEN** el reporte identifica las unidades y sus valores, conserva los resultados originales y aporta los metadatos necesarios para repetir el análisis sobre las mismas fuentes

#### Scenario: Herramienta o fuentes no disponibles
- **WHEN** faltan la herramienta requerida o las fuentes que el alcance declara analizar
- **THEN** el procedimiento informa que la medición no está disponible y no presenta valores cero ni cifras estimadas como resultados obtenidos

#### Scenario: Analisis incompleto
- **WHEN** se detectan archivos omitidos o sintaxis no reconocida por el analizador dentro del alcance declarado
- **THEN** el reporte identifica la limitación y no presenta la medición como una cobertura completa de ese alcance

### Requirement: Comparaciones ciclomaticas con alcance explicito
Una comparación ciclomática SHALL declarar si coinciden herramienta, convención y alcance. SHALL informar cambios en el número de unidades analizadas y distinguir métricas de unidades de sus agregados, sin sumar repetidamente contenedores e hijos ni presentar sumas o promedios locales como una medida universal de complejidad del sistema.

#### Scenario: Extraccion de funciones
- **WHEN** se comparan versiones que distribuyen decisiones entre un número diferente de funciones
- **THEN** la comparación informa ese cambio de unidades y no concluye una reducción de complejidad global basándose únicamente en una media local menor

#### Scenario: Cambio de analizador
- **WHEN** los reportes utilizan revisiones del analizador con reglas de conteo diferentes
- **THEN** la comparación declara la incompatibilidad directa o vuelve a medir ambas versiones con la misma configuración

### Requirement: Separacion entre metricas y evidencia de ejecucion
La demostración SHALL presentar las estadísticas del DAG, los presupuestos y el error numérico observado con sus unidades y alcance. La documentación SHALL distinguirlos de la complejidad ciclomática del código. Ninguna reducción de estas métricas SHALL presentarse por sí sola como prueba de aceleración, menor memoria real, cumplimiento de SOLID o corrección.

#### Scenario: Programas de distinto tamaño con el mismo evaluador
- **WHEN** se comparan dos programas de distinto tamaño evaluados por la misma versión del motor
- **THEN** se identifican por separado las estadísticas de cada programa y las métricas de esa versión del código, sin atribuir un cambio ciclomático al mero cambio de entrada

#### Scenario: Evidencia numerica limitada a fixtures
- **WHEN** la demostración satisface la tolerancia de las fixtures neuronales
- **THEN** el reporte identifica esas fixtures y sus errores observados sin extender el resultado a una garantía global de precisión

### Requirement: Analisis externo al funcionamiento del motor
La construcción, compilación, inspección y evaluación del motor SHALL funcionar sin instalar el analizador de código. Los reportes ciclomáticos SHALL servir como evidencia de revisión y SHALL NOT imponer un umbral numérico automático de aceptación dentro de este cambio.

#### Scenario: Entorno sin analizador
- **WHEN** se compila y ejecuta la biblioteca en un entorno donde el analizador de código no está instalado
- **THEN** sus operaciones y pruebas funcionales se ejecutan con normalidad y solo el procedimiento externo de métricas informa la ausencia de la herramienta
