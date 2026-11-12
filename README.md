# EMLLM

Redes neuronales expresadas únicamente con **One** y **Magma**, donde
`Magma(x, y) = exp(x) - ln(y)` y `y > 0`.

El núcleo Rust construye DAGs cerrados, los compila en orden topológico y los
evalúa en `f64`. Incluye síntesis racional acotada y una red 2-2-1 cuyos pesos,
entradas, sesgos y operaciones se expanden a One/Magma antes de evaluar.
La planificación y el seguimiento están en
[`implement-one-magma-eml`](openspec/changes/implement-one-magma-eml/) y
[`formalize-rust-core-contracts`](openspec/changes/formalize-rust-core-contracts/).

## Guías del núcleo

- [Arquitectura y contratos](docs/architecture.md): ciclo de vida
  `Arena → compile → Program → evaluate`, procedencia, snapshots inmutables,
  errores por etapa, aislamiento y criterios SOLID.
- [Estadísticas y complejidad](docs/complexity.md): `Program::stats()`, modelo
  de coste, evidencia de las fixtures y receta reproducible del analizador
  externo, con cobertura, limitaciones y reglas para comparar revisiones.

El análisis ciclomático es opcional para desarrollar y ejecutar el motor.
Sus cifras describen código fuente; V/E/R/D describen el DAG y los errores
numéricos describen las fixtures. Ninguna de estas métricas, por sí sola,
demuestra rendimiento, memoria real, precisión general o cumplimiento de SOLID.

## Ejecutar y verificar el núcleo

Requiere Rust con soporte para edición 2024. Verificado localmente con
`rustc 1.92.0 (ded5c06cf 2025-12-08)` en `aarch64-apple-darwin`.
La biblioteca usa únicamente la biblioteca estándar.

Desde la raíz del repositorio:

```sh
cargo check
cargo test --release
cargo test --doc -- --show-output
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
cargo run --release --example infer
```

La demostración construye los tres programas completos antes de evaluarlos.
Usa presupuestos explícitos de 100.000 nodos para construcción, compilación y
evaluación; informa V (nodos únicos), E (enlaces de operandos), R (posiciones de
raíz), D (profundidad en enlaces), la referencia independiente, el error absoluto
y la tolerancia. Termina con error si una fixture no satisface `1e-6`.

Resultados locales de la demostración en release:

| Entrada | Salida EML | (V, E, R, D) | Error absoluto observado |
| --- | --- | --- | --- |
| `[0, 0]` | 0.110599608464273 | (614, 1226, 1, 148) | 2.426e-14 |
| `[1/2, -1/2]` | 0.104508979170250 | (676, 1350, 1, 148) | 9.104e-15 |
| `[-1/2, 1/2]` | -0.080306139871040 | (676, 1350, 1, 150) | 4.829e-15 |

La arena compartida contiene 1.684 nodos después de construir las tres entradas.
Estos resultados proceden de este motor, no de un prototipo externo. Son
observaciones de las fixtures y del entorno indicado, no garantías generales de
precisión o rendimiento.

## Modelo y límites

- `expr`: arena append-only, referencias propias de una arena, internado por
  estructura y orden de operandos, inspección y presupuesto de construcción.
- `program`: validación y snapshot de nodos alcanzables. El recorrido es
  iterativo; las raíces conservan orden y duplicados. Construir una entrada
  nueva no modifica programas ya compilados.
- `synth`: racionales normalizados con `abs(p)<=2` y `1<=q<=2`, suma, negación,
  resta y producto mediante las [expansiones derivadas](docs/synthesis.md).
  Los enteros de la frontera son metadatos, nunca hojas ejecutables.
- `eval`: recibe únicamente `&Program` validado y ejecuta One/Magma. Los fallos
  de dominio, desbordamiento, no finitud y presupuesto se devuelven como errores
  identificables; los fallos numéricos señalan el nodo del programa compilado.
- `network`: red 2-2-1 con activación `exp(-z*z)`, parámetros racionales y
  validación de dimensiones antes de construir. La referencia convencional
  está fuera de la biblioteca, en pruebas y demostración.

Una estructura válida no certifica la validez numérica. `f64` puede sufrir
cancelación, subdesbordamiento y desbordamiento de intermedios, incluso cuando
el resultado real final sea pequeño. No se aplican clamps ni sustituciones por
aritmética convencional. Las tolerancias comprobadas son `1e-9` para los
racionales iniciales, `1e-8` para las operaciones escalares documentadas y `1e-6`
para las tres fixtures neuronales. No se promete precisión arbitraria,
aceleración, entrenamiento ni escalado a redes grandes.

## Informe técnico conceptual

El informe y el resumen visual recogen el diseño inicial; el estado de la
implementación y sus resultados locales se describen en las secciones anteriores.

- [Resumen visual en GitHub Pages](https://sugarfreedonoso.github.io/emllm/).
- [Fuente LaTeX con diagramas TikZ](docs/report/emllm.tex).
- [Descargar el PDF en Releases](https://github.com/SugarFreeDoNoSo/emllm/releases).
- [Compilaciones de GitHub Actions](https://github.com/SugarFreeDoNoSo/emllm/actions/workflows/report.yml).

Para compilar localmente, instala TeX Live con español, TikZ, Latin Modern y
`latexmk`, y ejecuta:

```sh
cd docs/report
mkdir -p build
latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -outdir=build emllm.tex
```

El PDF queda en `docs/report/build/emllm.pdf` (desde la raíz del repositorio).
GitHub Actions compila cambios del informe en `main`, pull requests y ejecuciones
manuales, y guarda el PDF como artefacto. Para publicar una versión documental:

```sh
git tag -a report-v0.1.0 -m "Informe conceptual EMLLM 0.1.0"
git push origin report-v0.1.0
```

Usa un tag nuevo para cada versión y actualiza la versión documental del `.tex`.
Los tags `report-v*` crean una release con el PDF y su checksum SHA-256 tras una
compilación satisfactoria. Una release existente se conserva al repetir el job.

## GitHub Pages

El resumen estático está en `docs/site/`. El workflow `pages.yml` lo publica al
cambiar esos archivos en `main`. La fuente de publicación de Pages debe estar
configurada como **GitHub Actions**. El sitio no necesita dependencias ni un build.
