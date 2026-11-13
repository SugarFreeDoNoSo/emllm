# Preparación reproducible de stories260K

## Entorno externo

El runtime Rust sigue usando solo `std`. Estas herramientas se ejecutan en un
entorno separado de desarrollo: CPython 3.12.12, PyTorch 2.9.1, NumPy 2.2.6 y
SentencePiece 0.2.1. `tools/requirements-tiny.lock` fija también las dependencias
transitivas y sus hashes para macOS aarch64. El lock fue generado con uv 0.13.5:

```sh
uv pip compile tools/requirements-tiny.in --python-version 3.12.12 --python-platform aarch64-apple-darwin --generate-hashes --output-file tools/requirements-tiny.lock
```

Para reproducir se usa el lock existente, sin resolver versiones nuevas:

```sh
uv venv --python 3.12.12 target/tiny-model/venv
uv pip sync --python target/tiny-model/venv/bin/python --require-hashes tools/requirements-tiny.lock
target/tiny-model/venv/bin/python -B tools/prepare_tiny.py
target/tiny-model/venv/bin/python -B tools/prepare_tiny.py --offline
target/tiny-model/venv/bin/python -B -m unittest discover -s tools/tests -v
```

## Fuentes y comprobaciones

[`tiny-model.sources.json`](../tools/tiny-model.sources.json) fija la revisión
`0bd21da7698eaf29a0d7de3992de8a46ef624add` de `karpathy/tinyllamas` y los SHA-256 de
`stories260K.pt`, `stories260K.bin`, `tok512.model` y `tok512.bin`. Las descargas
usan URLs `/resolve/<revision>/stories260K/<archivo>` y se guardan bajo
`target/tiny-model/sources/`. Un archivo existente de tamaño/hash incorrecto se
rechaza; no se sobrescribe para ocultar el fallo. `--offline` exige los cuatro
archivos locales y no hace peticiones de red.

La referencia de arquitectura se fija a `karpathy/llama2.c` en
`350e04fe35433e6d2941dce5a1f53308f87058eb`. El checkpoint contiene claves con prefijo
`_orig_mod.` procedentes de `torch.compile`; se elimina solo ese prefijo y se
rechazan nombres duplicados o inventarios distintos del esperado.

La inspección real confirma:

| Dato | Valor |
| --- | --- |
| Parámetros únicos | 260032 |
| Bloques / dimensión / MLP | 5 / 64 / 172 |
| Cabezas Q / KV / dimensión de cabeza | 8 / 4 / 8 |
| Vocabulario / contexto entrenado | 512 / 512 |
| Pesos | F32, contiguos y finitos |
| Matriz de salida | Comparte storage, offset y bits con embedding |

`norm_eps=1e-5` y `rope_theta=10000` proceden de los defaults del código de
referencia fijado: no están almacenados como campos en `model_args` del
checkpoint. El dropout de entrenamiento es 0.05; la futura inferencia usará
modo evaluación. La carga utiliza `torch.load(..., weights_only=True,
map_location="cpu")` y valida también la cabecera little-endian de llama2.c.

## Entradas fijadas antes de EML

[`tiny-model-inputs.json`](../tests/fixtures/tiny-model-inputs.json) contiene el
texto original, hash de tok512, política BOS/EOS y las dos listas completas.
Se incluye BOS=1, no se añade EOS=2 y las posiciones empiezan en cero.

- 8 IDs: `[1,403,407,261,378,432,383,286]`.
- 16 IDs: `[1,403,407,261,378,432,383,286,261,376,298,315,421,395,317,426]`.

El preparador retokeniza el texto únicamente para comprobar esa fixture y falla
si obtiene otros IDs. Los futuros adaptadores consumirán las listas ya fijadas.

## Salidas de preparación

En `target/tiny-model/prepared/` se generan:

- `model-info.json`: configuración, formas, hashes de cada tensor, transformaciones
  de nombres y alias. Los hashes de tensor usan bytes row-major F32 little-endian.
- `inputs.json`: copia canónica de las dos fixtures.
- `hashes.json`: identidad del contenido anterior, script y lock de dependencias.
- `environment.json`: versiones de Python/paquetes y plataforma de esta preparación.

El entorno se registra separado del contenido canónico. Ninguno de estos datos
acredita todavía una inferencia EML ni una comparación de motores.

## Verificación de preparación

La tarea 2.1 comprobó dos ejecuciones offline con archivos canónicos idénticos.
SHA-256 de `model-info.json`:
`ae94376dbeb9204f5fa8a82dc8a45b9ee7865320a5e12d6cb4264f04e43ca7cf`.
SHA-256 de `inputs.json`:
`9e335b3fe54734f42bcfe6306600b5bce9e10c76f45216e1868dca57007aeafa`.

Una copia del `.pt` con un bit alterado, conservando el tamaño, termina con
`hash_mismatch: stories260K.pt`, código de salida 1 y sin publicar metadata
parcial. Pasaron también las dos pruebas locales de corrupción, truncamiento y
datos extra. Evidencia: `target/analysis/tiny-input-preparation/`.

## Exportación canónica

```sh
target/tiny-model/venv/bin/python -B tools/import_model.py --offline
```

Se generan `target/tiny-model/canonical/model.emltiny`, `manifest.json` y la
metadata de preparación asociada. El [formato binario v1](tiny-model-format.md)
documenta cada byte, inventario, alias, límites y constantes. La exportación
comprueba los pesos contra los originales `.pt` y `.bin` y verifica su propio
round-trip antes de publicar el paquete.

Verificación de la tarea 2.2: 47 tensores de pesos propietarios coinciden bit a
bit con ambas fuentes; la salida conserva el alias a embedding. El paquete
tiene 52 entradas y 1.043.208 bytes, SHA-256
`434011a9e90db0bd15b39b80f11676c9b238b27ca8c526a9b0b87efc5b02ecd5`.
Dos exportaciones produjeron binario y manifiesto idénticos. Pasaron seis pruebas
locales de integridad/formato, incluidas corrupción, alias, valores no finitos y
límites. Evidencia: `target/analysis/tiny-canonical-export/`.

## Referencias y corpus (2.4)

`tools/reference.py` ejecuta el `model.py` original, fijado por revisión y SHA-256
en `tools/reference.sources.json`, y fórmulas explícitas independientes en F32 y
F64. El original usa su rama manual de atención, modo evaluación y CPU/un hilo;
los hooks observan salidas sin modificarlas. El archivo fuente permanece intacto.
La ruta F64 eleva pesos y constantes F32 exactamente una vez y audita dtype,
dispositivo y finitud en cada etapa; no aplica `.double()` al modelo original.

```sh
target/tiny-model/venv/bin/python -B tools/reference.py
target/tiny-model/venv/bin/python -B tools/reference.py --offline
```

Se generan `target/tiny-model/reference/report.json`, entorno separado y tres
trazas por prefijo: `original_f32`, `explicit_f32` y `explicit_f64`. Cada traza
tiene `values.bin` y `trace.json` con dtype, forma row-major, offset, tamaño y
SHA-256 por tensor. Scores/probabilidades guardan solo posiciones causales:
`[head, packed]`, con `packed=t*(t+1)/2+j`, `0<=j<=t`; no contienen máscaras
infinitas. Las fórmulas explícitas usan el primer score permitido como pivote.

Se verificaron 93 etapas del original contra F32 explícito y 213 etapas
explícitas por dtype y prefijo. No hubo divergencias con `1e-4 + 1e-4*abs(ref)`.
Ambas fixtures producen 512 logits finitos en los tres recorridos.

El corpus se fija antes de evaluar EML en `reference/corpus/`:

- `scalars.u32`: 280688 bits F32 distintos, ordenados como u32 little-endian.
- `scalars.json`: primera procedencia tensor/índice de cada valor y hash binario.
- `corpus.json`: 6599 operaciones escalares, 631 productos/reducciones completos
  de longitudes 8/64/172 y 208 componentes RMSNorm/softmax, con operandos por bits,
  procedencia y oráculos Python F64. SiLU aparece también como operación escalar.

La selección incluye los primeros cuatro, mínimo/máximo y menor magnitud no nula
de cada etapa; desempata por primer índice. Añade casos de mínima cancelación
relativa en residuales/proyecciones, todos los pesos/constantes distintos y todos
los operandos de las reducciones/componentes seleccionados. Las sumas del oráculo
de reducciones agrupan pares adyacentes en árbol, pasando la cola impar al nivel
siguiente. Su agrupación se distingue de los kernels matriciales de PyTorch.

SHA-256 del corpus: `d4c9207a3ea4e9b526bdc4c6b2a791cd6439586c853a8b58e6c3f9c2095677c5`;
de `scalars.u32`: `f63e3268b16a6127ac23e86ff95523894de184bffb4f4ad750fed8326f237b42`.
Estos son oráculos externos, todavía no resultados de aritmética EML.

## Reproducción y precisión de las referencias (2.5)

Con las fuentes y el paquete ya preparados, los siguientes comandos no descargan
archivos ni retokenizan las entradas de inferencia (el preparador comprueba la
fixture original antes de consumir sus IDs):

```sh
target/tiny-model/venv/bin/python -B tools/reference.py --offline --output-dir target/analysis/tiny-reference-repeat
target/tiny-model/venv/bin/python -B tools/verify_reference.py --compare-dir target/analysis/tiny-reference-repeat
```

La reproducción verificada produjo **16 archivos idénticos** excluyendo el
registro ambiental: seis pares de binario/manifiesto, corpus y reporte. Se
comprobaron 1038 tensores de etapa, 388939 referencias tensor/índice y la presencia
de todos los bits de pesos/constantes en el corpus. El reporte contiene los 512
logits de los tres recorridos para cada fixture. SHA-256 de `report.json`:
`a853b59e12d26d3a9517d17c489069dd0bef864ebaf4d0859b6e84066771ca3f`.

Entorno observado: macOS 26.7.2 arm64, CPython 3.12.12, PyTorch 2.9.1 y NumPy
2.2.6; CPU, un hilo intra-op y uno inter-op. OMP/MKL/OpenBLAS/Accelerate se fijan
a un hilo antes de importar las bibliotecas. Las operaciones deterministas están
activadas. `environment.json` registra los valores efectivos.

| Prefijo | F32 explícito vs original, máximo | F64 vs original, máximo | F64 vs original, RMS | Top-1 (tres rutas) |
| --- | ---: | ---: | ---: | ---: |
| 8 | 6.794930e-6 | 5.659565e-6 | 3.023752e-6 | 261 |
| 16 | 8.583069e-6 | 5.016644e-6 | 2.495596e-6 | 338 |

Los criterios permanecen fijados: constantes `1e-9 + 1e-7*abs(ref)`; componentes
`1e-6 + 1e-5*abs(ref)`; etapas/logits `1e-4 + 1e-4*abs(ref)`; diferencia entre
evaluadores EML `1e-12 + 1e-12*abs(ref)`. NaN/infinito invalida un caso completo.
Las métricas de esta tabla comparan todos los logits; top-1 es solo diagnóstico.
La máxima fracción del umbral entre etapas originales/F32 explícitas es 0.151924
(8) y 0.128250 (16). Entre las 213 etapas explícitas F64/F32 es 0.103418 y
0.182681, respectivamente. Todos los elementos pasan.

La comparación conserva diferencias legítimas: división por `sqrt(8)` en el
original frente a multiplicación por escala F32 canónica; softmax original con
su kernel frente a exponenciales respecto del primer score; SiLU original frente
a su fórmula explícita; acumulaciones F32/F64. No se cambian las tolerancias para
compensarlas. Los oráculos de componentes usan **operandos F32 de las trazas
elevados a F64**, mientras la referencia completa F64 propaga activaciones F64.

El corpus evidencia un rango amplio: mínimo `-48.26185607910156` en
`prefix16/layers.1.scores[489]`, máximo `2.781185048576e12` en
`prefix16/layers.1.score_exp[756]` y menor magnitud no nula `2.1137395426764934e-23`
en `prefix16/layers.1.probabilities[489]`. Estas magnitudes forman parte de la
puerta pendiente de síntesis; la finitud de PyTorch no demuestra que EML pueda
representarlas o ejecutar las operaciones dentro de los límites.
