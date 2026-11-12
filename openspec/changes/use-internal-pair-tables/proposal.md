# Proposal

## Why

La arena y los programas guardan actualmente un `Vec<Node>` con una etiqueta One/Magma por nodo, aunque el lenguaje tiene una única constante y todas las demás formas son pares ordenados de referencias. Una tabla interna de pares con One implícito puede eliminar ese almacenamiento redundante y conservar la compartición del DAG, con un coste estructural explícito y verificable.

## What Changes

- Sustituir el almacenamiento estructural de `Arena` y `Program` por tablas contiguas de pares de índices, reservando el identificador lógico 0 para One y almacenando una fila por Magma.
- Mantener la inicialización perezosa de One en la arena: una arena recién creada sigue vacía y One sigue consumiendo un nodo del presupuesto, aunque no ocupe una fila.
- Conservar identidad de arena, internado por operandos ordenados, compilación alcanzable iterativa, snapshots propietarios, raíces ordenadas y repetidas y errores por etapa.
- **BREAKING:** cambiar `nodes()` de slice a iterador de valores `Node` y `node()` a `Option<Node>` por copia; migrar conjuntamente consumidores, pruebas, doctests y documentación. El usuario ha aceptado esta adaptación para evitar mantener otra colección expandida.
- Añadir inspección de pares de solo lectura y conteo lógico explícito; conservar el enum One/Magma como vista lógica, no como almacenamiento duplicado del grafo.
- Evaluar directamente el programa de pares validado, con los mismos temporales `f64`, presupuestos, diagnósticos y tolerancias. El índice 0 designa One; no introduce una constante numérica cero.
- Registrar el coste de los elementos estructurales ocupados frente al modelo anterior, separándolo de capacidad reservada, memoria total del proceso, tiempo y complejidad ciclomática.

## Capabilities

### New Capabilities

- `eml-pair-table-storage`: representación interna compacta de DAGs One/Magma mediante pares, correspondencia entre filas e identificadores, inspección lógica sin materialización y evidencia acotada de almacenamiento.

### Modified Capabilities

Ninguna del inventario principal, que actualmente está vacío. `eml-expressions`, `eml-network-inference`, `eml-program-contracts` y `eml-complexity-reporting` pertenecen a los cambios previos; sus garantías semánticas se conservan. La nueva capacidad concreta el almacenamiento y la migración de firmas de inspección, sin relajar esos contratos.

## Impact

- **Dependencia:** aplicar después de completar y verificar `formalize-rust-core-contracts`, a su vez basado en `implement-one-magma-eml`. En la inspección inicial el primero está en 15/23, no completado; terminarlo corresponde a su propio cambio.
- **Código previsto:** `src/expr.rs`, `src/program.rs`, `src/eval.rs`, consumidores de inspección en pruebas, `src/lib.rs` y `examples/infer.rs`. Las expansiones de `synth` y las fórmulas de `network` se preservan.
- **Documentación:** README, documentación de la API, `docs/architecture.md` y la guía de complejidad que complete el cambio dependiente.
- **Compatibilidad:** cambios de fuente en la API de inspección del crate 0.1.0, aceptados expresamente; no se añade formato de archivo, serialización, importación de tablas externas ni ABI estable.
- **Recursos:** biblioteca estándar, índices `usize` y presupuestos explícitos existentes. No se promete ahorro de memoria total ni aceleración, ni se cambia la precisión numérica. La representación binaria inversa y el almacenamiento en disco quedan fuera de este incremento.
