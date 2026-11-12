# Tasks

## 1. Núcleo de expresiones

- [ ] 1.1 Crear proyecto Cargo de biblioteca con módulos para expresiones, síntesis, evaluación y red; verificar compilación nativa con cargo check.
- [ ] 1.2 Implementar arena One/Magma, referencias validadas, raíces e internado estructural; verificar pruebas de operandos ordenados, compartición, referencias ajenas y ausencia de ciclos construibles.
- [ ] 1.3 Implementar presupuestos de construcción y recorrido de inspección; verificar que un presupuesto pequeño devuelve error y que toda expresión aceptada contiene únicamente las dos formas permitidas.
- [ ] 1.4 Documentar semántica, identidad estructural y separación construcción/evaluación en documentación del núcleo; verificar que ejemplos de construcción no llaman al evaluador.

## 2. Evaluación EML

- [ ] 2.1 Implementar recorrido topológico iterativo y evaluación f64; verificar One, Magma(One,One) y expresiones compartidas contra valores independientes con las tolerancias especificadas.
- [ ] 2.2 Implementar errores de dominio, desbordamiento, valores no finitos y límites; verificar casos que fuerzan cada error y comprobar que incluyen el nodo responsable cuando corresponda.
- [ ] 2.3 Documentar precisión, cancelación y subdesbordamiento; verificar que no existen clamps ni sustituciones numéricas encubiertas en la ruta de evaluación.

## 3. Síntesis pura

- [ ] 3.1 Derivar y documentar expansiones One/Magma de negación, suma, producto y racionales acotados, con condiciones de positividad; verificar cada identidad algebraicamente antes de integrarla, sin inferirla solo de ejemplos numéricos.
- [ ] 3.2 Implementar las expansiones y normalización racional sin constantes ejecutables; verificar todos los racionales del dominio mínimo, denominador cero y presupuesto agotado.
- [ ] 3.3 Añadir pruebas independientes de suma, producto y negación para {-1/2,0,1/2}; verificar tolerancia 1e-8 y pureza de todos los nodos alcanzables, incluyendo expresiones compuestas.
- [ ] 3.4 Documentar las expansiones elegidas y sus límites numéricos con resultados nativos; verificar que cualquier fallo bloquea el avance y no se sortea mediante instrucciones aritméticas adicionales.

## 4. Inferencia de integración

- [ ] 4.1 Implementar constructor de red 2-2-1, activación exp(-z*z) y salida afín mediante las macros puras; verificar dimensiones inválidas y que todas las raíces se construyen antes de evaluar.
- [ ] 4.2 Implementar referencia aritmética independiente en pruebas y ejecutar las tres fixtures de la especificación; verificar error absoluto máximo 1e-6 y representación One/Magma de entradas, pesos y sesgos.
- [ ] 4.3 Añadir ejemplo ejecutable con número de nodos, presupuestos y error observado; verificar que una nueva entrada genera nuevas raíces y conserva el significado de las anteriores.
- [ ] 4.4 Actualizar README con comandos exactos de pruebas y ejemplo, alcance y límites; ejecutar los comandos documentados y registrar resultados locales sin reutilizar métricas del prototipo externo.

## 5. Validación conjunta

- [ ] 5.1 Ejecutar cargo test --release, cargo fmt --check y cargo clippy --all-targets -- -D warnings; verificar que pasan y que la demostración completa satisface ambas especificaciones.
