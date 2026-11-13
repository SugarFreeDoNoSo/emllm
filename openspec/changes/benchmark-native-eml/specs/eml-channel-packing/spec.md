# Spec Delta

## Purpose

Agrupar canales reales como pares de componentes sin cambiar el modelo, permitiendo contrastar su representación y sus costes con una disposición no empaquetada.

## ADDED Requirements

### Requirement: Correspondencia reversible de canales
El empaquetado SHALL asociar canales reales adyacentes a componentes real e imaginaria y conservar la dimensión original. Una matriz de pesos de forma `n × m` SHALL tener vista empaquetada `ceil(n/2) × m`. Desempaquetar SHALL recuperar cada expresión y su orden sin evaluación numérica.

#### Scenario: Matriz con numero par de filas
- **WHEN** se empaquetan y desempaquetan cuatro filas de expresiones distintas
- **THEN** se obtienen dos filas de pares y se recuperan las cuatro filas originales sin pérdida ni intercambio de componentes

### Requirement: Padding sin contribucion al modelo
Una dimensión impar SHALL representarse con una componente de relleno identificada y excluida del dominio lógico. El relleno SHALL NOT añadir pesos efectivos, cambiar divisores de normalización, introducir probabilidades ni aparecer en las salidas lógicas.

#### Scenario: Tres canales normalizados
- **WHEN** se empaquetan tres canales y se calcula su RMSNorm
- **THEN** se usan exactamente esos tres canales y divisor tres, y se devuelven tres resultados lógicos aunque existan dos pares físicos

### Requirement: Proyeccion real general preservada
La proyección con pesos y activaciones empaquetados SHALL reproducir la proyección real de forma compatible dentro del dominio numérico admitido, incluidas matrices que no representan aplicaciones lineales complejas. SHALL conservar todas las contribuciones entre canales y el orden de reducción real fijado.

#### Scenario: Matriz sin estructura compleja especial
- **WHEN** se proyecta la entrada real `[5,6]` con pesos `[[1,2],[3,4]]` mediante la disposición empaquetada
- **THEN** las componentes de salida corresponden a `[17,39]`, sin sustituir la proyección por una multiplicación compleja de menor información

### Requirement: No linealidades y puertas por componentes reales
RMSNorm, SiLU, las puertas SwiGLU y residuales SHALL conservar sus fórmulas reales, dimensiones y orden. El par SHALL NOT convertir una operación elemento a elemento en una operación compleja que mezcle ambas componentes. La aritmética numérica compleja trascendente queda fuera de este contrato.

#### Scenario: Componentes de signos diferentes
- **WHEN** se aplica SiLU y el producto de puertas a pares con componentes reales de signos distintos
- **THEN** cada componente coincide con la fórmula real correspondiente, sin términos cruzados de multiplicación compleja

### Requirement: Ejes de atencion y rotacion conservados
El empaquetado SHALL identificar el eje de canales y preservar posiciones, cabezas Q/KV, emparejamiento RoPE y causalidad. Los scores y probabilidades SHALL conservar su significado real y su eje de normalización; agrupar canales SHALL NOT mezclar posiciones ni normalizar sobre un número distinto de elementos.

#### Scenario: Rotacion de un par de canales
- **WHEN** se aplica RoPE a componentes `(a,b)` con coeficientes reales `(c,s)` fijados
- **THEN** se obtienen las expresiones de `(a*c-b*s, a*s+b*c)` en el mismo orden que la referencia

#### Scenario: Token futuro modificado
- **WHEN** se modifica un token posterior a una posición observada
- **THEN** las activaciones causales anteriores permanecen equivalentes en ambas disposiciones y las parejas Q siguen usando la cabeza KV correcta

### Requirement: Alias y propiedad conservados
Las vistas empaquetadas SHALL conservar los alias del modelo y la procedencia de sus componentes. Una operación de empaquetado SHALL NOT duplicar silenciosamente pesos propietarios ni convertir handles de otro contexto en referencias locales válidas.

#### Scenario: Embedding y proyeccion de salida
- **WHEN** ambas operaciones acceden a sus vistas empaquetadas del peso compartido
- **THEN** referencian las mismas expresiones de base y seleccionan las componentes correspondientes a cada coordenada original

### Requirement: Equivalencia de las dos disposiciones
La variante empaquetada SHALL satisfacer los criterios numéricos completos del modelo y la diferencia `1e-12 + 1e-12*abs(ref)` frente a la no empaquetada decodificada con el mismo evaluador. La equivalencia SHALL comprobarse en los 512 logits y en las etapas diagnósticas.

#### Scenario: Ejecucion completa de ambas variantes
- **WHEN** se componen y decodifican los dos prefijos fijados en ambas disposiciones
- **THEN** todas las comparaciones pasan sin cambiar pesos, datos, agrupaciones de sumas ni tolerancias

### Requirement: Contabilidad de componentes y trabajo real
Los informes SHALL distinguir pares físicos, canales reales, nodos, referencias, capacidades y bytes. SHALL incluir el coste de empaquetar, acceder o reordenar datos que ocurra por solicitud. Reducir el número de elementos de una vista SHALL NOT contarse por sí solo como reducción de memoria o cómputo.

#### Scenario: Mitad de elementos y mismos bytes
- **WHEN** una vista usa la mitad de elementos y cada elemento almacena dos componentes del mismo tamaño
- **THEN** se informa el almacenamiento completo y cualquier ahorro, ausencia de ahorro o aumento observado, sin aplicar un factor dos supuesto
