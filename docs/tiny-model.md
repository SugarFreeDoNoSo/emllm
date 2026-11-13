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
