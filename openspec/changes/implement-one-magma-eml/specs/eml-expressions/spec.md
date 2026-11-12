## Purpose

Representar cálculos EML como expresiones cerradas One/Magma, construidas antes de su evaluación y con límites numéricos explícitos.

## ADDED Requirements

### Requirement: Lenguaje cerrado One/Magma
El sistema SHALL representar cada programa ejecutable únicamente mediante `One` y `Magma(a,b)`. SHALL excluir constantes numéricas adicionales, variables, parámetros enlazados y operaciones aritméticas primitivas.

#### Scenario: Inspección de un programa
- **WHEN** se inspeccionan todos los nodos alcanzables de un programa construido
- **THEN** cada nodo es One o Magma y cada referencia resuelve a otro nodo del programa

### Requirement: Semántica EML
El evaluador SHALL interpretar One como 1 y Magma(a,b) como exp(a)-ln(b), con b estrictamente positivo. La evaluación numérica SHALL declarar su precisión y no confundirse con identidad estructural.

#### Scenario: Expresión elemental
- **WHEN** se evalúa Magma(One,One)
- **THEN** se obtiene e con error absoluto no mayor de 1e-12

### Requirement: Construcción separada de evaluación
El sistema SHALL permitir completar y validar un DAG antes de ejecutar exp o ln sobre sus nodos. SHALL compartir únicamente expresiones estructuralmente iguales, preservando el orden de operandos.

#### Scenario: Subexpresiones repetidas
- **WHEN** se construye dos veces Magma(One,One) en el mismo DAG
- **THEN** ambas referencias identifican una subexpresión compartida sin haber requerido evaluación numérica

### Requirement: Síntesis acotada y macros expandidas
El sistema SHALL convertir racionales p/q y constructores de suma, negación y producto a One/Magma sin hojas numéricas. SHALL admitir como mínimo p entre -2 y 2 y q entre 1 y 2. La igualdad algebraica SHALL distinguirse del error del evaluador.

#### Scenario: Racionales con signo
- **WHEN** se sintetizan 0, 1, -1, 1/2, -1/2 y 2
- **THEN** todos producen DAGs puros y valores con error absoluto no mayor de 1e-9

#### Scenario: Operaciones compuestas
- **WHEN** se construyen suma, negación y producto para operandos del conjunto {-1/2,0,1/2}
- **THEN** sus resultados tienen error absoluto no mayor de 1e-8 respecto de la aritmética de referencia y todos sus nodos son One/Magma

### Requirement: Fallos y presupuestos explícitos
El sistema SHALL rechazar denominadores cero, referencias inválidas, ciclos y presupuestos de construcción excedidos. SHALL devolver errores identificables cuando un argumento logarítmico no sea positivo, una operación desborde o aparezcan valores no finitos; SHALL NOT devolver esos fallos como resultados válidos.

#### Scenario: Error de dominio
- **WHEN** se evalúa una expresión cuyo segundo argumento de Magma vale cero o es negativo
- **THEN** se informa un error de dominio asociado al nodo

#### Scenario: Presupuesto agotado
- **WHEN** una expansión necesita más nodos que el máximo configurado
- **THEN** la construcción termina con un error de límite y no entrega un programa parcial como válido
