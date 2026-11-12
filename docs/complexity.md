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
