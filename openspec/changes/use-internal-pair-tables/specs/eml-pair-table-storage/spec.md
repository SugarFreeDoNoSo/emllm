# Spec Delta

## Purpose

Representar e inspeccionar programas One/Magma mediante tablas internas de pares que conserven la compartición y permitan comprobar su coste estructural sin alterar el significado numérico del lenguaje.

## ADDED Requirements

### Requirement: Correspondencia entre pares y nodos logicos
La representación SHALL exponer una tabla de pares ordenados de referencias de solo lectura. El identificador lógico 0 SHALL designar One sin ocupar fila; la fila k SHALL describir los operandos del Magma con identificador k+1. Una tabla validada SHALL contener una fila por Magma único y referencias a identificadores lógicos anteriores, sin introducir nuevas formas ejecutables.

#### Scenario: Programa de una constante
- **WHEN** se compila la raíz One
- **THEN** la tabla no contiene filas, la raíz es 0 y la inspección del identificador 0 devuelve One

#### Scenario: Dos niveles compartidos
- **WHEN** se compila Magma(t,t), donde t es Magma(One,One)
- **THEN** la tabla es [[0,0],[1,1]], la raíz es 2 y la inspección distingue tres nodos lógicos

### Requirement: Presencia de One y presupuesto logico
Una arena nueva SHALL comenzar sin nodos lógicos. Crear One SHALL requerir una unidad de presupuesto aunque no añada filas, y repetir su construcción SHALL reutilizar la referencia. Los límites de construcción, compilación y evaluación SHALL seguir contando nodos lógicos, incluido One, no solamente filas de pares.

#### Scenario: Arena vacia sin presupuesto
- **WHEN** se crea una arena con presupuesto cero y se solicita One
- **THEN** se devuelve un error de límite, la tabla sigue vacía y la inspección no encuentra ningún nodo, incluido el identificador 0

#### Scenario: One reutilizado en una arena llena
- **WHEN** se crea One en una arena con límite uno y se solicita de nuevo
- **THEN** ambas referencias coinciden, no hay filas y el conteo lógico permanece en uno

#### Scenario: Cero filas no equivale a coste logico cero
- **WHEN** se intenta compilar One con límite cero o evaluar su programa válido con límite cero
- **THEN** cada operación devuelve el error de presupuesto de su etapa aunque la tabla no tenga filas

### Requirement: Comparticion y procedencia conservadas
El internado de pares SHALL preservar el orden de operandos y reutilizar las referencias de estructuras idénticas. Las referencias de construcción SHALL conservar su procedencia y opacidad; el identificador reservado de One SHALL NOT permitir mezclar arenas. Un rechazo de procedencia al construir Magma SHALL ocurrir antes de insertar su fila.

#### Scenario: Pares repetidos y ordenados
- **WHEN** se construyen dos veces Magma(a,b) y una vez Magma(b,a), con a y b distintos
- **THEN** las construcciones repetidas comparten identificador y fila, y la construcción invertida ocupa otra fila

#### Scenario: Dos constantes de distintas arenas
- **WHEN** se usa como operando o raíz de una arena el handle de One de otra arena, ambas con índice local 0
- **THEN** se devuelve un error identificable de procedencia y no se entrega la construcción o programa solicitado

### Requirement: Almacenamiento proporcional al DAG compartido
La representación residente SHALL conservar pares de referencias sin desplegar subárboles compartidos ni mantener una segunda colección completa de nodos etiquetados. Inspeccionar, compilar y evaluar SHALL operar sin materializar el árbol expandido. Las estructuras auxiliares de validación, internado y valores numéricos SHALL identificarse separadamente del almacenamiento de pares.

#### Scenario: Cadena de duplicacion compartida
- **WHEN** se construye t0=One y, para i de 1 a 20000, ti=Magma(ti-1,ti-1), y se compila t20000 con presupuesto suficiente
- **THEN** la tabla compilada tiene 20000 filas y 20001 nodos lógicos, y la compilación e inspección completan mediante recorrido iterativo sin expandir el árbol ni evaluar sus valores

### Requirement: Inspeccion logica por valor
La inspección lógica SHALL permitir consultar un nodo por índice como valor independiente y recorrer los nodos mediante un iterador con longitud exacta, en orden de identificador. La consulta fuera de rango SHALL devolver ausencia. Obtener una consulta o un iterador SHALL NOT asignar una colección completa de nodos; cambiar una copia obtenida SHALL NOT modificar la representación.

#### Scenario: Lectura consistente con la tabla
- **WHEN** se inspecciona el programa de tabla [[0,0],[1,1]] por índice y mediante iteración
- **THEN** ambas vías producen One, Magma(0,0), Magma(1,1) en ese orden, el iterador empieza con longitud tres y la consulta del índice tres devuelve ausencia

#### Scenario: Copia independiente
- **WHEN** un consumidor obtiene un nodo por valor y modifica su copia local
- **THEN** consultas posteriores, pares, raíces y estadísticas del programa conservan sus valores originales

### Requirement: Encapsulacion de las vistas de pares
Las tablas y raíces publicadas SHALL ser de solo lectura. La API segura SHALL NOT permitir fabricar un programa validado ni referencias de construcción a partir de pares o índices arbitrarios. Cada snapshot SHALL poseer su tabla alcanzable y conservarla después de ampliar o liberar su arena de origen.

#### Scenario: Intento de escribir una fila
- **WHEN** un consumidor externo intenta modificar una fila a través de la vista pública de pares
- **THEN** su programa no compila

#### Scenario: Independencia de la arena
- **WHEN** se compila un programa, se amplía la arena y después se libera
- **THEN** los pares, raíces y nodos lógicos del snapshot anterior siguen siendo inspeccionables y conservan su significado

### Requirement: Compilacion alcanzable de tablas
La compilación SHALL seleccionar únicamente las dependencias alcanzables, remapear sus índices y conservar orden y duplicados de raíces. SHALL rechazar raíces vacías o ajenas, referencias inexistentes y ciclos detectados en fixtures internas sin entregar un programa parcial. La validación de tablas SHALL completar sin evaluar operaciones numéricas.

#### Scenario: Filas excluidas y raices repetidas
- **WHEN** una arena contiene One, t=Magma(One,One), u=Magma(One,t) y v=Magma(t,t), y se compilan las raíces [v,One,v]
- **THEN** la tabla del programa es [[0,0],[1,1]], las raíces son [2,0,2] y la fila de u queda excluida

#### Scenario: Tabla interna malformada
- **WHEN** una fixture interna contiene una referencia inexistente, una autorreferencia o un ciclo entre filas alcanzables
- **THEN** la compilación devuelve la categoría estructural correspondiente y no entrega un programa validado

### Requirement: Evaluacion con identificadores logicos
La evaluación SHALL consumir el programa de pares validado con One en la posición lógica 0, conservar el orden de salidas y los errores numéricos existentes y señalar el identificador lógico del nodo fallido. SHALL devolver todas las salidas o un error. La representación compacta SHALL NOT cambiar las fórmulas, añadir clamps ni usar resultados de la referencia convencional.

#### Scenario: Raices simples repetidas
- **WHEN** se evalúa la tabla [[0,0]] con raíces [1,0,1] y presupuesto de dos nodos
- **THEN** se obtienen tres salidas en ese orden, la primera y tercera corresponden al mismo valor de e con error máximo 1e-12 y la segunda es 1

#### Scenario: Error numerico tras el desplazamiento de filas
- **WHEN** se evalúa un programa bien formado que provoca dominio negativo o desbordamiento
- **THEN** se devuelve el error con un identificador lógico resoluble por inspección, y sus pares y raíces siguen intactos sin entregar salidas parciales exitosas

### Requirement: Estadisticas logicas independientes de filas
Las estadísticas SHALL conservar sus definiciones: V cuenta nodos lógicos únicos, E posiciones de operandos, R posiciones de raíz y D profundidad máxima en enlaces. En un programa validado con M filas SHALL cumplirse V=M+1 y E=2M. Las raíces repetidas SHALL NOT añadir filas. Consultar estadísticas SHALL NOT evaluar el programa.

#### Scenario: One y subexpresiones compartidas
- **WHEN** se consultan los programas One, t=Magma(One,One), Magma(t,t) y raíces [t,t]
- **THEN** sus tuplas (V,E,R,D) son respectivamente (1,0,1,0), (2,2,1,1), (3,4,1,2) y (2,2,2,1), con 0, 1, 2 y 1 filas

### Requirement: Evidencia acotada del almacenamiento
El reporte de almacenamiento SHALL identificar revisión, target, tamaño de índice, número de filas y bytes de sus elementos ocupados, junto al modelo equivalente de nodos etiquetados. SHALL distinguir ese payload de capacidad reservada, metadatos, internado, raíces y temporales numéricos. SHALL NOT presentar una reducción del payload como medición de memoria total, aceleración o precisión.

#### Scenario: Comparacion de representaciones
- **WHEN** se reporta un programa con M filas, V nodos y tamaño de índice W bytes
- **THEN** el payload de pares es 2*M*W bytes y el modelo de elementos etiquetados es V veces el tamaño de esa forma en el target, con exclusiones declaradas y sin cifras atribuidas a memoria total

### Requirement: Regresion de las fixtures del lenguaje
La migración SHALL conservar las tolerancias de aceptación de los racionales, operaciones escalares y red existentes, además de sus errores y presupuestos. Los resultados SHALL compararse con referencias independientes; preservar estructura o reducir almacenamiento SHALL NOT considerarse prueba de precisión numérica.

#### Scenario: Dominio escalar inicial
- **WHEN** se ejecutan las fixtures de racionales p/q con -2<=p<=2 y 1<=q<=2 y las operaciones documentadas sobre {-1/2,0,1/2}
- **THEN** los errores absolutos siguen dentro de 1e-9 para racionales y 1e-8 para operaciones y composiciones

#### Scenario: Red 2-2-1 original
- **WHEN** se ejecutan las tres fixtures neuronales originales con entradas [0,0], [1/2,-1/2] y [-1/2,1/2], en ese orden
- **THEN** cada salida satisface el error absoluto máximo de 1e-6 y se informan separadamente sus estadísticas, presupuestos y payload estructural
