# Tasks

## 1. Dependencia de implementacion

- [x] 1.1 Confirmar que `implement-one-magma-eml` está implementado y satisface sus especificaciones; verificar la presencia del crate, registrar su revisión y los resultados de sus comprobaciones y fixtures. Si la base falta o falla, detener la aplicación de este cambio e informar esa dependencia.

## 2. Referencias y frontera estructural

- [x] 2.1 Encapsular las referencias con procedencia de arena y conservar identidad sin reutilización; verificar operandos y raíces de otra arena con posiciones locales iguales, además de un doctest negativo de fabricación de referencias y su control positivo.
- [x] 2.2 Consolidar la compilación iterativa de nodos alcanzables con presupuesto explícito y orden topológico; verificar lista de raíces vacía, orden y duplicados, compartición, presupuesto insuficiente y fixtures internas con ciclos o referencias inexistentes.
- [x] 2.3 Introducir o consolidar el programa validado como snapshot propietario e inmutable; verificar que sigue siendo inspeccionable y evaluable tras ampliar y liberar la arena original, y que un consumidor externo no puede fabricar ni modificar sus campos mediante la API segura.
- [x] 2.4 Exponer inspección de solo lectura e identificadores de nodos del programa para diagnósticos; verificar que los operandos compartidos se reconocen como el mismo nodo y que todos los identificadores publicados resuelven dentro del snapshot correspondiente.
- [x] 2.5 Documentar construcción, compilación, procedencia, raíces y propiedad del snapshot en la API; ejecutar sus doctests positivos y `compile_fail`, revisar los diagnósticos de los ejemplos negativos y comprobar que ninguno depende de imports o preparación inválidos.

## 3. Evaluacion aislada y composicion

- [x] 3.1 Hacer que la entrada pública de evaluación acepte exclusivamente el programa validado y migrar consumidores y ejemplos de la base; verificar mediante doctests que la ruta válida compila y que pasar una arena o referencia aislada directamente no compila.
- [ ] 3.2 Consolidar errores distinguibles por etapa y categoría, con identificación de nodo cuando corresponda y éxito solo con todas las salidas; verificar un programa de varias raíces con fallo numérico y errores previos a disponer de nodo, sin resultados parciales exitosos.
- [ ] 3.3 Aislar buffers y configuración por evaluación y conservar presupuestos explícitos; verificar evaluaciones intercaladas, reintento tras presupuesto insuficiente y preservación de raíces y nodos, usando tolerancias de fixtures en lugar de exigir igualdad bit a bit de trascendentes.
- [ ] 3.4 Verificar la separación estructural/numérica con las expresiones de dominio negativo y desbordamiento descritas en `design.md`; comprobar que ambas compilan estructuralmente, fallan en evaluación con la categoría y nodo esperados y siguen siendo inspeccionables después del fallo.
- [ ] 3.5 Consolidar las dependencias de expresiones, síntesis, programa, evaluación y red, conservando los nombres coherentes de la base; verificar por revisión de imports y llamadas que síntesis y compilación no ejecutan el evaluador y que la referencia convencional solo se usa en pruebas/demostración.
- [ ] 3.6 Crear `docs/architecture.md` y la documentación de errores de la API con los contratos y criterios SRP, OCP, LSP, ISP y DIP; verificar que explican enum cerrado, composición funcional, encapsulación, elección justificada de traits y límites de pureza/precisión de `f64` con ejemplos alineados con la API real.

## 4. Inspeccion y reporte del programa

- [ ] 4.1 Calcular y exponer las estadísticas estructurales V, E, R y D al compilar; verificar los valores exactos de One, Magma(One,One), Magma(t,t) y raíces [t,t] establecidos en `eml-complexity-reporting`.
- [ ] 4.2 Verificar exclusión de nodos no alcanzables y estabilidad de estadísticas tras ampliar/liberar la arena o fallar una evaluación; añadir además un caso de cadena profunda dentro del presupuesto que complete compilación e inspección sin depender de recursión ni de evaluación numérica.
- [ ] 4.3 Ampliar la demostración neuronal para informar estadísticas por programa junto a presupuestos y errores observados; ejecutar las tres fixtures originales y comprobar que conservan su tolerancia de 1e-6 y el orden de sus salidas.
- [ ] 4.4 Documentar la API de estadísticas e iniciar `docs/complexity.md` con sus definiciones y el modelo O(V + E + R); verificar que los ejemplos coinciden con los casos de prueba y distinguen almacenamiento modelado, medidas reales y alcance de las fixtures.

## 5. Medicion reproducible del codigo

- [ ] 5.1 Seleccionar y fijar una versión publicada o revisión Git exacta de `rust-code-analysis-cli` que reconozca las fuentes reales del núcleo; verificar cobertura de archivos y errores de parsing y registrar la instalación reproducible y la procedencia exacta en `docs/complexity.md`.
- [ ] 5.2 Completar la receta documental de exportación JSON y metadatos bajo `target/analysis/`, incluyendo revisión, cambios locales, fuentes adicionales, comando y alcance; ejecutar los comandos documentados y verificar que los resultados por archivo y unidad pueden vincularse con esas fuentes.
- [ ] 5.3 Generar el primer reporte del núcleo y repetir la receta con las mismas fuentes y revisión del analizador; verificar consistencia de los valores por unidad y documentar número de unidades, máximo observado, tratamiento de cierres, pruebas inline, macros y agregados sin doble conteo.
- [ ] 5.4 Documentar y comprobar las salidas del procedimiento ante herramienta ausente, fuentes ausentes o análisis incompleto; verificar que esos estados no se presentan como métricas cero y que la biblioteca y sus pruebas funcionales siguen funcionando sin el analizador instalado.
- [ ] 5.5 Añadir a la guía un ejemplo interpretativo de extracción de funciones y las reglas para comparar revisiones; verificar que identifica cambios de denominador/convención y no presenta menor complejidad local como prueba de rendimiento, corrección o cumplimiento de SOLID.

## 6. Integracion y cierre de implementacion

- [ ] 6.1 Enlazar ambas guías desde README y verificar que sus nombres de API, rutas y comandos corresponden a los entregables reales, que cubren los escenarios de las dos especificaciones nuevas y que identifican las limitaciones numéricas y de medición.
- [ ] 6.2 Ejecutar `cargo test --release`, `cargo test --doc`, `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings`; comprobar que pasan los contratos nuevos y las regresiones de racionales, operaciones y red de `implement-one-magma-eml`, y registrar los resultados junto al reporte de complejidad.

## Flujo de ejecución

1. Usar `develop` como única rama de desarrollo para las tareas de `implement-one-magma-eml` y `formalize-rust-core-contracts`.
2. Completar y verificar primero `implement-one-magma-eml`; continuar después con `formalize-rust-core-contracts` sobre esa misma rama.
3. Crear un commit por tarea completada y verificada, incluyendo sus cambios y la actualización de su casilla en `tasks.md`. Completar ese commit antes de avanzar a la siguiente tarea.
4. Identificar el cambio y el número de tarea en cada mensaje de commit; formato de referencia: `<tipo>(<cambio>): tarea <X.Y> - <resumen>`.
5. Publicar los avances en `origin/develop` y revisar con el usuario antes de realizar el merge a `main`.

## Evidencia de la dependencia (1.1)

- Base: `f667704143f9539803f816010b9d53764623393d`, publicada en `develop`, árbol limpio al iniciar este cambio. `implement-one-magma-eml`: 16/16 y estado `all_done`; validación OpenSpec estricta satisfactoria.
- Crate `emllm` 0.1.0, edición 2024, biblioteca estándar, con `expr`, `synth`, `program`, `eval` y `network`, pruebas y `examples/infer.rs`.
- Comprobaciones ejecutadas sobre el código de esa revisión: `cargo check`, `cargo test --release` (22 pruebas y 1 doctest), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo run --release --example infer`, todas satisfactorias. Los commits finales de la base solo actualizaron README y seguimiento.
- Entorno: Rust 1.92.0, `aarch64-apple-darwin`. Errores máximos escalares: racionales `5.551e-17` (límite `1e-9`) y operaciones/composiciones `2.220e-14` (límite `1e-8`).
- Red: entradas `[0,0]`, `[1/2,-1/2]`, `[-1/2,1/2]`; nodos alcanzables 614, 676, 676; errores `2.426e-14`, `9.104e-15`, `4.829e-15`, todos bajo `1e-6`. Arena compartida: 1.684 nodos. Evidencia local, sin garantía global de precisión.

## Revisión de diagnósticos (2.5)

`cargo test --doc -- --show-output`: 7 ejemplos satisfactorios. Los intentos de fabricar `NodeId` y `Program` fallaron por campos privados (E0451); las asignaciones a vistas de nodos y raíces, por referencias compartidas (E0594). Los tres controles positivos compilaron y se ejecutaron. Ningún fallo dependió de imports o preparación incorrectos. Estos códigos se revisaron en Rust 1.92.0; `compile_fail` en estable no los comprueba automáticamente.

En 3.1, `cargo test --doc eval::evaluate -- --show-output` verificó el control positivo y tres rechazos E0308: `Arena`, `NodeId` y `Vec<Node>` no son `Program`. Los consumidores de la base ya usan exclusivamente programas compilados.
