# Spec Delta

## Purpose

Permitir que los consumidores del núcleo Rust construyan, validen y evalúen programas mediante contratos públicos que preserven sus invariantes estructurales y hagan explícitos los fallos numéricos.

## ADDED Requirements

### Requirement: Evaluacion restringida a programas validados
La API pública de evaluación SHALL aceptar únicamente valores cuyo tipo acredite la validación estructural del programa. Una arena en construcción, una colección de nodos sin validar o una referencia aislada SHALL NOT ser entradas ejecutables de esa API.

#### Scenario: Consumidor intenta evaluar una construccion
- **WHEN** un consumidor externo intenta pasar directamente una arena en construcción a la API pública de evaluación
- **THEN** su programa no compila por incompatibilidad de tipos

#### Scenario: Consumidor evalua un programa validado
- **WHEN** un consumidor construye One, valida su raíz y entrega el programa resultante al evaluador con presupuesto suficiente
- **THEN** el programa compila y la evaluación devuelve el valor 1

### Requirement: Procedencia de referencias
Toda operación pública que acepte referencias de construcción SHALL comprobar que pertenecen a la arena receptora. Las referencias SHALL ser opacas y no fabricables desde índices arbitrarios mediante la API pública segura.

#### Scenario: Operando de otra arena
- **WHEN** se intenta construir Magma con un operando de otra arena, aunque ambas referencias ocupen la misma posición local
- **THEN** se devuelve un error identificable de procedencia y no se incorpora ese nodo

#### Scenario: Raiz de otra arena
- **WHEN** se solicita compilar una raíz que pertenece a otra arena
- **THEN** se devuelve un error identificable de procedencia y no se entrega un programa validado

#### Scenario: Referencia fabricada por un consumidor
- **WHEN** un consumidor externo intenta construir una referencia escribiendo sus campos internos o convirtiendo un índice arbitrario mediante la API pública segura
- **THEN** el intento no compila

### Requirement: Garantias estructurales del programa
La obtención de un programa validado SHALL garantizar que sus nodos alcanzables son One/Magma, sus referencias resuelven dentro del programa y sus dependencias son acíclicas y evaluables en orden. Un fallo de validación SHALL NOT producir un valor público que acredite esas garantías.

#### Scenario: Dependencias compartidas
- **WHEN** se compila una raíz Magma cuyos dos operandos comparten una subexpresión válida
- **THEN** el programa permite identificar esa misma subexpresión en ambos operandos y sus dependencias preceden a sus usos en el orden de evaluación

#### Scenario: Estructura invalida detectada
- **WHEN** la validación encuentra una referencia inexistente o un ciclo en una estructura de prueba interna
- **THEN** devuelve un error estructural identificable sin entregar un programa validado

### Requirement: Correspondencia de raices y resultados
La compilación SHALL exigir al menos una raíz y preservar el orden y las repeticiones de la lista solicitada. Una evaluación exitosa SHALL devolver un resultado por posición de esa lista.

#### Scenario: Raices repetidas y ordenadas
- **WHEN** se compilan las raíces [Magma(One,One), One, Magma(One,One)] y se evalúan correctamente
- **THEN** se devuelven tres resultados en ese orden, con el primero y el tercero iguales y el segundo igual a 1

#### Scenario: Lista de raices vacia
- **WHEN** se solicita compilar una lista vacía de raíces
- **THEN** se devuelve un error estructural identificable

### Requirement: Programa inmutable e independiente del constructor
El programa validado SHALL conservar sus nodos, raíces y significado después de nuevas construcciones o de liberar la arena de origen. La API pública segura SHALL NOT permitir fabricar directamente un programa validado ni mutar sus nodos o raíces.

#### Scenario: Arena ampliada y liberada
- **WHEN** se obtiene un programa, se añaden expresiones a su arena de origen y después se libera esa arena
- **THEN** el programa anterior sigue siendo inspeccionable y evaluable con las mismas raíces y significado

#### Scenario: Intento de invalidar el programa
- **WHEN** un consumidor externo intenta fabricar un programa validado mediante sus campos o modificar sus nodos o raíces
- **THEN** el intento no compila

### Requirement: Validacion independiente de la evaluacion numerica
La construcción y la validación estructural SHALL completar su trabajo sin evaluar exp o ln sobre los nodos. La obtención de un programa validado SHALL NOT prometer positividad de argumentos logarítmicos, finitud de intermedios ni cumplimiento de tolerancias numéricas.

#### Scenario: Programa bien formado con dominio invalido
- **WHEN** se compila una expresión One/Magma bien formada cuyo segundo argumento de un Magma resulta estrictamente negativo al evaluarse
- **THEN** la compilación tiene éxito y la evaluación posterior devuelve un error de dominio asociado al nodo responsable

#### Scenario: Programa bien formado con desbordamiento
- **WHEN** se compila una cadena One/Magma bien formada que provoca desbordamiento exponencial durante su evaluación
- **THEN** la compilación tiene éxito y la evaluación posterior devuelve un error de desbordamiento asociado al nodo responsable

### Requirement: Configuracion explicita y aislamiento entre ejecuciones
Los presupuestos aplicables SHALL recibirse explícitamente en cada etapa. Una evaluación SHALL NOT leer ni alterar el estado numérico de otras ejecuciones ni modificar el programa. El aislamiento SHALL NOT interpretarse como una garantía de identidad bit a bit para las funciones trascendentes; las tolerancias se verificarán para las fixtures documentadas.

#### Scenario: Evaluaciones intercaladas
- **WHEN** se evalúa Magma(One,One), se ejecuta otro programa que falla y se vuelve a evaluar el primero con presupuesto suficiente
- **THEN** ambas evaluaciones de Magma(One,One) satisfacen el error absoluto máximo de 1e-12 respecto de e y su inspección estructural no cambia

#### Scenario: Presupuestos distintos
- **WHEN** una evaluación falla por un presupuesto insuficiente y luego se repite con presupuesto suficiente
- **THEN** la segunda ejecución opera sobre el programa intacto y su resultado no depende del fallo anterior

### Requirement: Errores identificables sin resultados parciales validos
Los fallos públicos SHALL distinguir su etapa y categoría sin analizar mensajes de texto. Los fallos numéricos de un nodo SHALL identificarlo dentro del programa inspeccionable. Una operación fallida SHALL NOT devolver un programa parcial ni un conjunto parcial de salidas como si fueran un éxito.

#### Scenario: Falla una de varias raices
- **WHEN** se evalúa un programa con varias raíces y una de sus dependencias provoca un error numérico
- **THEN** se devuelve el error de evaluación con categoría e identificación del nodo y no una respuesta exitosa que omita las salidas fallidas

#### Scenario: Fallo previo a disponer de un nodo
- **WHEN** se rechaza una lista de raíces vacía o un presupuesto antes de ejecutar nodos
- **THEN** se informa la etapa y categoría correspondientes sin inventar un identificador de nodo
