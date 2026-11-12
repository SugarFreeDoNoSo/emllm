## Purpose

Construir y ejecutar una inferencia neuronal pequeña como un único DAG cerrado One/Magma, verificable contra una referencia independiente.

## ADDED Requirements

### Requirement: Red completamente expandida
El sistema SHALL construir una red densa 2-2-1 con activación exp(-z*z) en la capa oculta y salida afín. Entradas, pesos, sesgos y operaciones SHALL convertirse a One/Magma antes de evaluar cualquiera de sus salidas.

#### Scenario: Inferencia pura
- **WHEN** se construye la red con entradas y parámetros racionales admitidos
- **THEN** todas las raíces y nodos alcanzables pertenecen a un único DAG cerrado sin primitivas aritméticas ni enlaces numéricos

### Requirement: Verificación numérica independiente
La demostración SHALL comparar su salida contra evaluación convencional independiente con error absoluto máximo de 1e-6 para los casos documentados. SHALL informar número de nodos, error observado y límites utilizados, sin atribuir resultados del prototipo externo a esta implementación.

#### Scenario: Casos reproducibles
- **WHEN** se ejecuta la demostración con W1=[[1/2,-1/2],[0,1/2]], b1=[0,-1/2], Wout=[[1/2,-1/2]], bout=[0] y entradas [0,0], [1/2,-1/2] y [-1/2,1/2]
- **THEN** las tres salidas satisfacen la tolerancia y se comprueba la pureza estructural del programa

### Requirement: Entradas sin enlaces dinámicos
El sistema SHALL representar cada nueva entrada mediante nuevas expresiones cerradas, permitiendo compartir subexpresiones inmutables. SHALL rechazar dimensiones incompatibles antes de la evaluación.

#### Scenario: Cambio de entrada
- **WHEN** se solicita otra inferencia con una entrada diferente
- **THEN** se construyen las raíces correspondientes sin mutar hojas numéricas del programa anterior

#### Scenario: Dimensión incorrecta
- **WHEN** se proporciona una entrada con una dimensión distinta de la esperada
- **THEN** se devuelve un error de forma sin iniciar la evaluación
