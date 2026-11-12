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

## Receta de captura y exportación

Requiere Python 3, Git y la instalación anterior. Ejecutar desde la raíz sin
editar fuentes durante la captura. Elegir un `RUN` nuevo en cada ejecución:
el procedimiento rechaza directorios existentes para no mezclar resultados.
El alcance es **todos los `.rs` bajo `src`**, incluidas pruebas inline, sin
`tests/`, `examples/` ni expansión de macros. Los archivos de `src` registrados
en HEAD también se esperan: una ausencia accidental no reduce el inventario.
Una eliminación intencional requiere una nueva revisión o declarar otro alcance.

La copia `snapshot/src/` es la entrada efectiva del CLI. Sus rutas relativas
mantienen la correspondencia de `name`, líneas y unidades con los JSON; se
guardan todos sus bytes y hashes, incluidas fuentes nuevas o ignoradas por Git.
`changes.patch` conserva cambios registrados respecto de HEAD, y `git-status`
identifica además archivos sin seguimiento. Los archivos adicionales fuera de
`src` aparecen en el estado, pero no forman parte del alcance analizado.

```sh
export RUN="${RUN:-target/analysis/core-initial}"
export RCA="${RCA:-target/analysis/tools/rca-37e5d83/bin/rust-code-analysis-cli}"
python3 - <<'PY'
import hashlib, json, os, shutil, subprocess, sys
from datetime import datetime, timezone
from pathlib import Path

root = Path.cwd()
out = Path(os.environ['RUN']).resolve()
out.mkdir(parents=True, exist_ok=False)
cli = Path(os.environ['RCA']).resolve()
meta = {
    'created_utc': datetime.now(timezone.utc).isoformat(),
    'scope': 'src/**/*.rs; includes inline tests; excludes tests/ and examples/',
    'analyzer_revision': '37e5d83c056c8cbf827223d5814a93c5218df1a9',
    'analyzer_binary': str(cli), 'commands': [],
}

def record(label, command, cwd=root):
    result = subprocess.run(command, cwd=cwd, capture_output=True)
    (out / (label + '.stdout')).write_bytes(result.stdout)
    (out / (label + '.stderr')).write_bytes(result.stderr)
    meta['commands'].append({'label': label, 'argv': command,
                             'cwd': str(cwd), 'exit': result.returncode})
    return result

def finish(state, reason, **details):
    (out / 'metadata.json').write_text(json.dumps(meta, indent=2) + '\n')
    coverage = dict(state=state, reason=reason, **details)
    (out / 'coverage.json').write_text(json.dumps(coverage, indent=2) + '\n')
    print(json.dumps(coverage))
    sys.exit(0 if state == 'complete' else 1)

meta['source_revision'] = record('revision', ['git', 'rev-parse', 'HEAD']).stdout.decode().strip()
record('git-status', ['git', 'status', '--porcelain=v1', '--untracked-files=all'])
diff = record('git-diff', ['git', 'diff', '--binary', 'HEAD'])
(out / 'changes.patch').write_bytes(diff.stdout)
tracked = record('tracked', ['git', 'ls-tree', '-r', '--name-only', 'HEAD', '--', 'src'])
expected = set(p for p in tracked.stdout.decode().splitlines() if p.endswith('.rs'))
expected.update(str(p) for p in Path('src').rglob('*.rs'))
expected = sorted(expected)
meta['expected_files'] = expected
if not cli.is_file() or not os.access(cli, os.X_OK):
    finish('unavailable', 'analyzer absent or not executable')
if not expected or any(not Path(p).is_file() for p in expected):
    finish('unavailable', 'sources absent', missing=[p for p in expected if not Path(p).is_file()])
meta['analyzer_sha256'] = hashlib.sha256(cli.read_bytes()).hexdigest()
receipt = cli.parent.parent / '.crates2.json'
if receipt.is_file():
    shutil.copyfile(receipt, out / 'cargo-install.json')
for label, command in [('analyzer-version', [str(cli), '--version']),
                       ('rustc', ['rustc', '-Vv']), ('cargo', ['cargo', '-V'])]:
    if record(label, command).returncode:
        finish('unavailable', label + ' failed')
snapshot = out / 'snapshot'
hashes = {}
try:
    for name in expected + ['Cargo.toml', 'Cargo.lock']:
        data = Path(name).read_bytes()
        dest = snapshot / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        hashes[name] = hashlib.sha256(data).hexdigest()
except OSError as error:
    finish('unavailable', str(error))
(out / 'sources.sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')
cc = out / 'cc'
cc.mkdir()
metrics = record('metrics', [str(cli), '-m', '-p', 'src', '-I', '*.rs',
                            '-O', 'json', '-o', str(cc)], snapshot)
syntax = record('syntax', [str(cli), '-p', 'src', '-I', '*.rs', '-f', 'error'], snapshot)
try:
    reports = [json.loads(p.read_text()) for p in sorted(cc.rglob('*.json'))]
    emitted = sorted(report['name'] for report in reports)
except (OSError, ValueError, KeyError) as error:
    finish('incomplete', 'invalid JSON: ' + str(error))
if (metrics.returncode or syntax.returncode or metrics.stderr.strip()
        or syntax.stderr.strip() or syntax.stdout.strip() or emitted != expected):
    finish('incomplete', 'inspect metrics/syntax logs and file inventory',
           expected=expected, emitted=emitted)
finish('complete', 'matching inventory; no parser errors',
       expected=expected, emitted=emitted)
PY
```

La propiedad `analyzer_revision` declara la revisión requerida; contrastarla con
`cargo-install.json` y el hash del binario de la instalación seleccionada. No
atribuir ese SHA de fuentes a un ejecutable arbitrario llamado igual. Se retienen
los stdout/stderr originales, argv, directorio de ejecución, códigos de salida,
revisión completa de Git y versiones del toolchain. `coverage.json` acredita el
estado de esa captura, no el de una ejecución anterior.

Para reconstruir las entradas, usar directamente `snapshot/` y comprobar los
hashes de `sources.sha256.json`. También se puede recuperar `source_revision`,
aplicar `changes.patch` y restaurar las fuentes adicionales desde el snapshot.
Para cada unidad, conservar archivo, ruta de espacios padres, `kind`, `name`,
`start_line` y `end_line`: los nombres de métodos y cierres no son únicos.
El JSON original conserva todas las métricas y la jerarquía `spaces`.

La receta se ejecutó en `target/analysis/core-initial` sobre
`69b2d7745bb8306d751713d251b25e9e16ea7280`, con cambios documentales registrados
y sin cambios en `src`. Se verificaron los ocho hashes de fuentes/configuración,
la cobertura 6/6 y los rangos de los 82 espacios JSON contra sus fuentes
capturadas; sus identidades jerárquicas fueron únicas. «Espacios» incluye
contenedores y no equivale al número de funciones.

## Lectura por unidad y repetición

En esta revisión del analizador, `kind: function` representa tanto funciones y
métodos como cierres. `nom` permite distinguirlos; los cierres se nombran
`<anonymous>` y necesitan archivo, padres y líneas para identificarse. Los
espacios `unit`, `impl` y `trait` son contenedores, no funciones adicionales.

`metrics.cyclomatic.sum` **incluye los subespacios**. Para obtener el valor local
de un espacio se resta la suma exportada de sus hijos inmediatos, cada uno una
vez. Esta es una proyección aritmética del JSON, no otro analizador de Rust.
Por ejemplo, `compile_nodes` exporta 13, que incluye un cierre de 1: su valor
local es 12. Sumar 13 y 1 contaría el cierre dos veces. Los promedios exportados
por contenedor incluyen también espacios contenedores en el denominador;
no son automáticamente promedios por función.

Para generar una tabla derivada sin alterar los JSON originales:

```sh
export RUN="${RUN:-target/analysis/core-initial}"
python3 - <<'PY'
import json, os
from pathlib import Path

out = Path(os.environ['RUN'])
coverage = json.loads((out / 'coverage.json').read_text())
if coverage['state'] != 'complete':
    raise SystemExit('No report: ' + coverage['state'])
rows = []

def walk(file, space, parents=()):
    identity = [space[k] for k in ('kind', 'name', 'start_line', 'end_line')]
    children = space['spaces']
    cc = space['metrics']['cyclomatic']['sum']
    local = cc - sum(c['metrics']['cyclomatic']['sum'] for c in children)
    if space['kind'] == 'function':
        closures = space['metrics']['nom']['closures'] - sum(
            c['metrics']['nom']['closures'] for c in children)
        rows.append(dict(file=file, parents=list(parents), identity=identity,
                         category='closure' if closures == 1 else 'function/method',
                         local=local, exported_sum=cc))
    for child in children:
        walk(file, child, parents + (identity,))

for path in sorted((out / 'cc').rglob('*.json')):
    data = json.loads(path.read_text())
    walk(data['name'], data)
summary = {}
for file in coverage['expected']:
    values = [r['local'] for r in rows if r['file'] == file]
    summary[file] = dict(units=len(values), maximum=max(values, default=None),
                         local_sum=sum(values),
                         mean=sum(values) / len(values) if values else None)
(out / 'units.json').write_text(json.dumps(rows, indent=2) + '\n')
(out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
PY
```

Repetir la captura con `RUN=target/analysis/core-repeat` y ejecutar también esta
proyección con ese `RUN`. Antes de comparar `units.json`, exigir igualdad de
`sources.sha256.json`, revisión y hash del analizador, alcance y cobertura
completa en ambas ejecuciones. Comparar identidades y valores, no orden de
archivos generado en paralelo ni marcas de tiempo. Una nueva revisión Git que
solo cambie documentación puede tener las mismas fuentes; conservar ambas
revisiones en los metadatos, sin confundirlas.

### Convención y límites de lectura

- Cada espacio parte de 1; la regla Rust incrementa por nodos `if`, `for`,
  `while`, `loop`, brazos de `match`, expresión `?`, `&&` y `||`.
  Es una convención sintáctica de esa revisión, no un CFG del compilador.
  Por ejemplo, el token `for` de `impl Trait for Type` afecta al contenedor.
- Las funciones de los módulos `#[cfg(test)]` inline se analizan aunque una
  compilación de producción las excluya. Se mantienen en este alcance y se
  identifican en el reporte; no se deduce «producción» del nombre del archivo.
- Los doctests en comentarios no son unidades ejecutables del análisis.
  `tests/` y `examples/` requieren capturas adicionales con alcance declarado.
- No hay expansión de `derive`, macros declarativas/procedurales ni selección
  de `cfg` como en rustc. Los tokens visibles en una macro pueden influir en
  la métrica, pero no equivalen a medir su expansión; funciones o cierres
  dentro de token trees pueden no aparecer como unidades independientes.
- La cobertura completa significa que todos los archivos declarados emitieron
  JSON sin errores de parsing detectados. No significa cobertura de todo el
  código ejecutable que produciría rustc, ni determina pruebas suficientes.

Reglas consultadas en el commit fijado:
[`cyclomatic.rs`](https://github.com/mozilla/rust-code-analysis/blob/37e5d83c056c8cbf827223d5814a93c5218df1a9/src/metrics/cyclomatic.rs),
[`spaces.rs`](https://github.com/mozilla/rust-code-analysis/blob/37e5d83c056c8cbf827223d5814a93c5218df1a9/src/spaces.rs),
[`checker.rs`](https://github.com/mozilla/rust-code-analysis/blob/37e5d83c056c8cbf827223d5814a93c5218df1a9/src/checker.rs)
y [`getter.rs`](https://github.com/mozilla/rust-code-analysis/blob/37e5d83c056c8cbf827223d5814a93c5218df1a9/src/getter.rs).

### Primer reporte verificado

`core-initial` (HEAD `69b2d77`) y `core-repeat` (HEAD `0b29531`) conservaron los
mismos ocho hashes de fuentes/configuración y el mismo binario. Ambos dieron
cobertura 6/6, sin errores de parsing; coincidieron los valores de las 58
unidades y todos los JSON al compararlos como datos. Las revisiones completas,
cambios documentales y fuentes están registrados en cada captura.

| Archivo | Funciones/métodos | Cierres | Pruebas inline incluidas | Máximo local | Suma local | Media por unidad |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `src/eval.rs` | 6 | 2 | 3 | 6 | 21 | 2.625 |
| `src/expr.rs` | 16 | 0 | 5 | 5 | 35 | 2.1875 |
| `src/lib.rs` | 0 | 0 | 0 | — | 0 | — |
| `src/network.rs` | 8 | 0 | 0 | 7 | 26 | 3.25 |
| `src/program.rs` | 11 | 1 | 3 | 12 | 34 | 2.8333 |
| `src/synth.rs` | 14 | 0 | 0 | 11 | 61 | 4.3571 |
| **Alcance completo** | **55** | **3** | **11** | **12** | **177** | **3.0517** |

Las pruebas inline son un subconjunto de las 55 funciones, no unidades para
añadir otra vez. Se identifican en los módulos de prueba de `eval.rs:143`,
`expr.rs:167` y `program.rs:220` del snapshot. Los tres cierres pertenecen a
`evaluate`, `magma_value` y `compile_nodes`. Los 24 espacios restantes del JSON
son seis archivos y 18 bloques `impl`; se excluyen de este denominador de 58.
`lib.rs` fue analizado correctamente y no contiene funciones en este alcance:
su suma vacía no es una medición ausente ni un máximo igual a cero.

El máximo local observado es **12**, en `src/program.rs:157-218`,
`compile_nodes`; `mul` (`synth.rs:137-149`) y `rational`
(`synth.rs:152-172`) tienen 11. La suma 177 y la media 177/58 son resúmenes de
estas unidades bajo la convención anterior, no complejidad universal del
sistema, umbrales de calidad ni evidencia de rendimiento. Los detalles
reproducibles quedan en `units.json`, `summary.json` y `cc/` de cada captura.

## Medición no disponible o incompleta

La captura termina con código 1 y un `coverage.json` explícito cuando falta una
condición. La proyección se niega a producir `units.json`/`summary.json` si su
estado no es `complete`. Nunca sustituir una ausencia por complejidad cero ni
usar el resumen de otra captura como si correspondiese a la fallida.

| Condición | Estado | Evidencia a inspeccionar |
| --- | --- | --- |
| Ejecutable ausente o no ejecutable | `unavailable` | Ruta de `RCA` y razón en `coverage.json` |
| Alcance vacío o fuente esperada ausente/ilegible | `unavailable` | Inventario esperado, lista de ausencias o error de lectura |
| JSON inválido, archivo omitido/duplicado o fallo del comando | `incomplete` | `metrics.stdout`, `metrics.stderr`, argv e inventarios |
| Error de parsing, aun con exit code 0 y todos los JSON | `incomplete` | `syntax.stdout` con archivos y rangos afectados |

Una advertencia en stderr se trata conservadoramente como análisis incompleto:
revisar su causa antes de publicar cifras. Resolver la condición y crear un
`RUN` nuevo; no editar un estado a mano para convertirlo en `complete`.

### Controles ejecutados

Se ejecutó la misma receta en una copia controlada de las fuentes bajo
`target/analysis/procedure-checks/`, con un directorio nuevo por caso:

- `missing-tool`: `RCA` apuntaba a un archivo inexistente; produjo
  `unavailable`, sin cifras.
- `missing-source`: se retiró de esa copia `src/expr.rs`, registrado en su
  HEAD; produjo `unavailable` e identificó exactamente esa ausencia.
- `omitted-file`: un wrapper de prueba añadió `-X '*expr.rs'` al CLI real;
  produjo `incomplete` al comparar seis archivos esperados con cinco emitidos.
  Este wrapper es una inyección controlada de omisión, no el analizador de los
  reportes del núcleo.
- `syntax-error`: una fuente adicional `src/invalid_control.rs` con
  `fn broken( {` produjo JSON y error de parsing. Aunque la cobertura de archivos
  fue 7/7 y el CLI terminó con cero, la receta produjo `incomplete`.

Las cuatro ejecuciones devolvieron 1 y sus proyecciones rechazaron el resumen;
conservan los logs originales y `projection.stderr`. Los controles de omisión
y sintaxis no son mediciones del núcleo. Para repetirlos, usar una copia
descartable, nunca retirar o invalidar las fuentes de trabajo del repositorio.

Antes de introducir esos defectos, la copia aislada del crate, sin el binario en
su directorio ni en `PATH`, ejecutó `cargo test --release`: **29 pruebas y 12
doctests satisfactorios**. Los logs `without-analyzer.stdout` y
`without-analyzer.stderr` están junto a los controles. La biblioteca estándar
es la única dependencia del motor; la ausencia del CLI afecta solo a la
receta externa y no impone una barrera de ejecución ni un umbral de CI.
