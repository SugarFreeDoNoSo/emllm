# Benchmark de composición nativa EML

## Estado y frontera

`benchmark-native-eml` investiga la composición simbólica de stories260K: modelo
preparado y símbolos de entrada → **512 expresiones One/Magma de logits**. La
preparación depende de longitud y disposición, sin recibir los tokens concretos.
Cada solicitud crea su estado mutable; el intervalo nativo incluye validación,
selección de embeddings, asociación de puertos, instanciación, internado,
materialización de raíces y liberación de temporales internos.

Carga, preparación, codificación, compilación para lectura, planificación,
evaluación numérica, comparación, selección del token y emisión tienen tiempos
propios fuera de ese intervalo. Se publicará también el extremo a extremo con
alcance explícito. El cociente composición/evaluación numérica no acredita una
aceleración de igual trabajo ni latencia de generación de tokens.

La aceptación requiere disposiciones no empaquetada/emparejada, PyTorch, llama2.c
y llama.cpp, ambos prefijos originales, tres calentamientos y diez muestras por
combinación. Cada muestra debe validarse numéricamente fuera del cronómetro.
Los pares agrupan canales reales y conservan ambas componentes y su orden;
su número de elementos no implica ahorrar el 50 % de memoria o trabajo.

Actualmente está verificada la base de datos y referencias (tarea 1.1); todavía
no hay preparación nativa ni inferencia completa de este experimento.
[`benchmark-tiny-model-eml`](../openspec/changes/benchmark-tiny-model-eml/tasks.md)
conserva **13/29 tareas**, con su límite histórico de 10000000 nodos descrito en
[tiny-model.md](tiny-model.md#revision-de-la-puerta-numerica-completa-y-bloqueo-de-recursos).
El seguimiento nativo está en [sus tareas](../openspec/changes/benchmark-native-eml/tasks.md).

## Base observada (2026-11-13, tarea 1.1)

- HEAD: `4e22ab502a46565966d0c0ce338f8cd9a22fb2ad`; rama `develop`, sincronizada
  con `origin/develop` al iniciar. Único contenido local pendiente: planificación
  aprobada de `benchmark-native-eml`.
- Apple M4 Pro, 14 CPU físicas/lógicas, RAM 25769803776 bytes (24 GiB).
- macOS 26.7.2, build 25G332, `aarch64-apple-darwin`.
- rustc 1.92.0 (`ded5c06cf`, LLVM 21.1.3), cargo 1.92.0 (`344c4567c`).
- CPython 3.12.12, PyTorch 2.9.1, NumPy 2.2.6, SentencePiece 0.2.1.
  Runtime Rust exclusivamente `std`; entorno numérico externo fijado por lock.
- Datos y fuente original de referencias disponibles offline. Los adaptadores
  temporizados de llama2.c/llama.cpp y la revisión/conversión F32 de llama.cpp
  quedan por implementar en 6.3/6.4; no son dependencias verificadas en esta fase.

Modelo: cinco bloques, dimensión 64, MLP 172, cabezas Q/KV 8/4, dimensión de
cabeza 8, vocabulario/contexto 512/512. Se comprobaron los 260032 parámetros
únicos, alias embedding/salida, 52 entradas canónicas y 292930 escalares contando
alias y auxiliares, conservando cada bit F32.

Fuentes inmutables: tinyllamas `0bd21da7698eaf29a0d7de3992de8a46ef624add` y
llama2.c `350e04fe35433e6d2941dce5a1f53308f87058eb`.

| Contenido | SHA-256 verificado |
| --- | --- |
| `tools/tiny-model.sources.json` | `3c03c5173230647bdc69e42951f7a0a4b2a5b99563c77748bbc445ea36daab2d` |
| `tools/reference.sources.json` | `b25c86a4011492f4c87de435406a570873802c29cdda54c4addf893129be4f0e` |
| `tools/requirements-tiny.lock` | `dbffd374db7375f8ee7a5657fc88cedd840cf8d51a3dcdba8be6f8d31f17391d` |
| `tests/fixtures/tiny-model-inputs.json` | `41747d9f09b3efcec3588513aece83bee35c4982dc18c14079660f310c13b333` |
| `canonical/model.emltiny` (1043208 bytes) | `434011a9e90db0bd15b39b80f11676c9b238b27ca8c526a9b0b87efc5b02ecd5` |
| `canonical/manifest.json` | `a181ea7bec75ba1475092f9d4c0c1ca052ea74573338a6082b3cf21d3b23f87f` |
| `reference/report.json` | `a853b59e12d26d3a9517d17c489069dd0bef864ebaf4d0859b6e84066771ca3f` |
| `reference/corpus/corpus.json` | `d4c9207a3ea4e9b526bdc4c6b2a791cd6439586c853a8b58e6c3f9c2095677c5` |

Los caminos `canonical/` y `reference/` son relativos a `target/tiny-model/`.
Los manifiestos de fuentes verifican también tamaño y hash de cada descarga.
El paquete canónico contiene bits, todavía no un programa One/Magma ejecutable.

Prefijos originales, BOS=1 incluido, sin EOS, posiciones desde cero:

- 8: `[1,403,407,261,378,432,383,286]`.
- 16: `[1,403,407,261,378,432,383,286,261,376,298,315,421,395,317,426]`.

## Verificación de la base

Comandos ejecutados antes de modificar el núcleo, desde la raíz del repositorio:

```sh
cargo test --release
target/tiny-model/venv/bin/python -B tools/prepare_tiny.py --offline --output-dir target/analysis/native-baseline/prepared
target/tiny-model/venv/bin/python -B tools/import_model.py --offline --output-dir target/analysis/native-baseline/canonical
target/tiny-model/venv/bin/python -B tools/verify_model_io.py
target/tiny-model/venv/bin/python -B tools/reference.py --offline --output-dir target/analysis/native-baseline/reference
target/tiny-model/venv/bin/python -B tools/verify_reference.py --compare-dir target/analysis/native-baseline/reference
target/tiny-model/venv/bin/python -B -m unittest discover -s tools/tests -v
```

Resultado: **60 pruebas Rust, 20 doctests y 10 pruebas Python** satisfactorias.
Paquete y manifiesto reexportados idénticos byte a byte; lector Rust idéntico
por bits y metadatos. Referencias reproducidas: **16 archivos idénticos** salvo
entorno, 1038 tensores de etapa, 388939 referencias de procedencia y 280688
patrones escalares distintos. Evidencia local en `target/analysis/native-baseline/`.

Los 512 logits de cada referencia pasan; máximo F64/original F32:
`5.65956483478125e-6` (8 IDs) y `5.0166442999710625e-6` (16 IDs).
Top-1 de las tres rutas: 261 y 338. La referencia explícita F64 mantiene sus
propias fórmulas/dtype; no equivale a aplicar `.double()` al original.

Tolerancias conservadas por elemento:

| Comparación | Límite de error absoluto |
| --- | --- |
| Constantes | `1e-9 + 1e-7*abs(ref)` |
| Componentes | `1e-6 + 1e-5*abs(ref)` |
| Etapas/logits | `1e-4 + 1e-4*abs(ref)` |
| Evaluadores EML / disposiciones | `1e-12 + 1e-12*abs(ref)` |

Se mantienen también las tolerancias originales del núcleo (1e-9, 1e-8, 1e-6).
Valores no finitos o un logit fuera de tolerancia invalidan el caso aunque top-1
coincida. Estas comprobaciones de referencias no son resultados de inferencia EML.
