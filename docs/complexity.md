# Estadísticas y complejidad

Esta guía distingue el tamaño de un programa One/Magma, el coste modelado de
procesarlo y las métricas del código que lo implementa. Los contratos de tipos,
errores y precisión se explican en [arquitectura](architecture.md).

## Estadísticas del programa

`Program::stats()` devuelve `&ProgramStats`, una vista de solo lectura calculada
durante la compilación. Consultarla cuesta O(1), no recorre el grafo ni ejecuta
exp/ln. Sus campos son:

| Campo | Símbolo | Definición y unidad |
| --- | --- | --- |
| `nodes` | V | Nodos únicos alcanzables desde las raíces |
| `edges` | E | Posiciones de operando de Magma; dos hijos iguales cuentan dos enlaces |
| `roots` | R | Posiciones de raíz, incluidos duplicados |
| `depth` | D | Máxima profundidad en enlaces; One tiene profundidad 0 |

Cada Magma tiene profundidad `1 + max(profundidad_izquierda, profundidad_derecha)`.
Se calcula en orden topológico sin recursión. Los nodos no alcanzables de la
arena no contribuyen a ningún conteo. La propiedad del snapshot hace que estos
valores no cambien al ampliar/liberar la arena o fallar una evaluación.

Casos exactos cubiertos por `tests/statistics.rs`, con `t=Magma(One,One)`:

| Raíces | V | E | R | D |
| --- | ---: | ---: | ---: | ---: |
| `[One]` | 1 | 0 | 1 | 0 |
| `[t]` | 2 | 2 | 1 | 1 |
| `[Magma(t,t)]` | 3 | 4 | 1 | 2 |
| `[t,t]` | 2 | 2 | 2 | 1 |

El doctest de `ProgramStats` usa el tercer caso. Una cadena de 20.001 nodos
verifica V=20.001, E=40.000, R=1 y D=20.000 sin intentar su evaluación numérica.

## Modelo de coste y alcance

En un modelo de operaciones de tamaño fijo, el recorrido estructural cuesta
O(V + E + R), suponiendo coste esperado constante para accesos a los mapas hash.
No es una garantía de peor caso para esos mapas. Cada nodo se procesa una vez;
las posiciones de operando y raíz también tienen coste. La evaluación almacena
V valores y produce R salidas. Como cada Magma tiene dos operandos, nodos,
temporales y raíces requieren almacenamiento lineal O(V + R).

Este modelo no mide bytes del asignador, capacidad reservada de vectores/tablas,
RSS, cachés del procesador o duración de exp/ln. La arena y su internado, incluidos
nodos no alcanzables, tienen costes adicionales. Cada snapshot posee sus copias;
compartir subexpresiones dentro de un programa no implica compartir almacenamiento
entre programas. Los presupuestos de nodos tampoco limitan por sí solos R,
tiempo de CPU o bytes. Una lista larga de raíces repetidas puede tener V pequeño
y aun así requerir espacio y trabajo para R posiciones.

Cambiar una entrada puede cambiar V/E/R/D mientras se ejecuta la misma versión
del evaluador. Eso no cambia la complejidad ciclomática de su código fuente.
Tampoco menos nodos, menor profundidad o menos decisiones locales demuestran
por sí solos aceleración, menor memoria real, precisión o cumplimiento de SOLID.

## Demostración neuronal

```sh
cargo run --release --example infer
```

La revisión `f81d08f` informó lo siguiente en Rust 1.92.0,
`aarch64-apple-darwin`, perfil release:

| Entrada | V | E | R | D | Error absoluto observado |
| --- | ---: | ---: | ---: | ---: | ---: |
| `[0,0]` | 614 | 1226 | 1 | 148 | 2.426e-14 |
| `[1/2,-1/2]` | 676 | 1350 | 1 | 148 | 9.104e-15 |
| `[-1/2,1/2]` | 676 | 1350 | 1 | 150 | 4.829e-15 |

Los límites de construcción, compilación y evaluación fueron 100.000 nodos;
la arena compartida terminó con 1.684 nodos. Todas las salidas satisfacen `1e-6`
frente a la referencia independiente. Son observaciones de estas tres fixtures,
no una cota global de precisión ni un benchmark de rendimiento. Los errores de
dominio y desbordamiento siguen siendo posibles en otros programas válidos.

## Analizador externo y procedencia

Se seleccionó `rust-code-analysis-cli` del repositorio
[`mozilla/rust-code-analysis`](https://github.com/mozilla/rust-code-analysis),
commit **`37e5d83c056c8cbf827223d5814a93c5218df1a9`**. Esta revisión usa edición
2024, `tree-sitter = 0.25.3` y `tree-sitter-rust = 0.23.2`. El ejecutable informa
`0.0.25`, igual que una publicación anterior con otras dependencias: ese número
solo no identifica el analizador utilizado.

Instalación ejecutada desde la raíz, con Cargo/Rust 1.92.0 y un compilador C
disponible para las gramáticas de Tree-sitter:

```sh
cargo install --git https://github.com/mozilla/rust-code-analysis \
  --rev 37e5d83c056c8cbf827223d5814a93c5218df1a9 --locked \
  --root target/analysis/tools/rca-37e5d83 \
  --bin rust-code-analysis-cli rust-code-analysis-cli
target/analysis/tools/rca-37e5d83/bin/rust-code-analysis-cli --version
```

Cargo descarga también los submódulos del repositorio. Además, avisa que ese
commit no publica `Cargo.lock`: **`--locked` no fija aquí las dependencias
transitivas**. La receta fija las fuentes del analizador y su gramática Rust;
no promete reconstruir un binario idéntico. Para repetir exactamente esta
medición se conserva el ejecutable local y se identifica por SHA-256, junto a
la salida de instalación y el registro `.crates2.json` de Cargo. Una instalación
nueva debe repetir la comprobación de compatibilidad y registrar su propio hash.
Estos archivos son evidencia de desarrollo bajo `target/analysis/`, ignorada
por Git; conservar ese directorio si se necesita comparar después.

Binario utilizado en `aarch64-apple-darwin`:
`8ca02c1b389db24ec87a43ed04d3e7f64c9567897c67305aed81b078bbd53351`.
El motor no tiene dependencias Cargo del analizador ni lo invoca.

### Compatibilidad comprobada (tarea 5.1)

Sobre las fuentes de `5c87381`, sin modificaciones locales en `src`, se ejecutó:

```sh
mkdir -p target/analysis/compatibility/cc
target/analysis/tools/rca-37e5d83/bin/rust-code-analysis-cli \
  -m -p src -I '*.rs' -O json -o target/analysis/compatibility/cc
target/analysis/tools/rca-37e5d83/bin/rust-code-analysis-cli \
  -p src -I '*.rs' -f error
```

Se comparó el inventario `src/**/*.rs` con el campo `name` de todos los JSON:
**6/6 archivos** (`eval`, `expr`, `lib`, `network`, `program`, `synth`), sin
omisiones, duplicados, stderr ni nodos con error de parsing. Un control separado
con `fn broken( {` sí produjo `ERROR` y su localización, aunque el CLI devolvió
código de salida cero. Por eso el procedimiento comprueba tanto la cobertura
como el contenido de la salida de `-f error`; el exit code por sí solo no basta.
La búsqueda usa `has_error()` del árbol de sintaxis, incluidos errores en hijos.
Esto acredita estas fuentes reales, no toda la sintaxis posible de Rust 2024.

El JSON y los logs de esta comprobación están en
`target/analysis/compatibility/`. Referencias del procedimiento:
[exportación de métricas](https://mozilla.github.io/rust-code-analysis/commands/metrics.html),
[búsqueda de errores](https://mozilla.github.io/rust-code-analysis/commands/nodes.html)
y [fuentes fijadas del CLI](https://github.com/mozilla/rust-code-analysis/tree/37e5d83c056c8cbf827223d5814a93c5218df1a9/rust-code-analysis-cli).
