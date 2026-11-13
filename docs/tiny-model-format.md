# Paquete canónico EMLTINY v1

El contenedor transporta metadata entera y bits F32, no nodos ejecutables. Todos
los enteros y escalares usan little-endian. No hay padding, texto ni datos
dependientes del tamaño nativo de `usize`. La procedencia y SHA-256 se guardan
en `manifest.json`, fuera del binario.

## Cabecera: 64 bytes

| Offset | Tipo | Campo |
| --- | --- | --- |
| 0 | 8 bytes | Magic `45 4d 4c 54 49 4e 59 00` (`EMLTINY\0`) |
| 8 | u32 | Versión, exactamente 1 |
| 12 | u32 | Cantidad de entradas N |
| 16 | u32 | Dimensión del estado |
| 20 | u32 | Dimensión intermedia MLP |
| 24 | u32 | Bloques |
| 28 | u32 | Cabezas Q |
| 32 | u32 | Cabezas KV |
| 36 | u32 | Vocabulario |
| 40 | u32 | Contexto máximo del checkpoint |
| 44 | u32 | Posiciones cubiertas por las constantes RoPE |
| 48 | u64 | Offset del directorio, exactamente 64 |
| 56 | u64 | Offset del payload, exactamente `64 + 48*N` |

Todas las dimensiones son positivas. El contenedor permite fixtures pequeñas;
la carga del modelo objetivo exige además la configuración
`64,172,5,8,4,512,512,16`, inventario y formas de stories260K.

## Entrada de directorio: 48 bytes

Los offsets de esta tabla son relativos al inicio de cada entrada:

| Offset | Tipo | Campo |
| --- | --- | --- |
| 0 | u32 | Tipo de tensor |
| 4 | u32 | Capa; `0xffffffff` para un tensor global |
| 8 | u32 | Rango, 0/1/2 |
| 12 | u32 | Índice del alias anterior, o `0xffffffff` para datos propios |
| 16 | u64 | Primera dimensión, 1 si rango 0 |
| 24 | u64 | Segunda dimensión, 1 si rango menor de 2 |
| 32 | u64 | Offset absoluto del primer byte de datos |
| 40 | u64 | Bytes del tensor, `4 * producto(dimensiones utilizadas)` |

Un escalar de rango 0 ocupa cuatro bytes. Dimensiones utilizadas, tamaños y
offsets se comprueban sin overflow antes de indexar o reservar. Cada dato es un
F32 finito; se preservan los bits, incluido el signo de cero.

| Tipo | Tensor | Forma stories260K |
| --- | --- | --- |
| 1 | `tok_embeddings.weight` | `[512,64]` |
| 2 | `attention_norm.weight` | `[64]` |
| 3 | `attention.wq.weight` | `[64,64]` |
| 4 | `attention.wk.weight` | `[32,64]` |
| 5 | `attention.wv.weight` | `[32,64]` |
| 6 | `attention.wo.weight` | `[64,64]` |
| 7 | `ffn_norm.weight` | `[64]` |
| 8 | `feed_forward.w1.weight` | `[172,64]` |
| 9 | `feed_forward.w2.weight` | `[64,172]` |
| 10 | `feed_forward.w3.weight` | `[172,64]` |
| 11 | `norm.weight` | `[64]` |
| 12 | `output.weight` | `[512,64]`, alias de entrada 0 |
| 13 | `rope.cos` | `[16,4]` |
| 14 | `rope.sin` | `[16,4]` |
| 15 | `norm.epsilon` | escalar |
| 16 | `attention.scale` | escalar |

Solo los tipos 2..10 llevan capa: índice en `0..n_layers`. Los demás usan la
marca global. Cada pareja tipo/capa aparece una sola vez. El paquete objetivo
tiene N=52: embedding; tipos 2..10 de cada capa en orden; tipos 11..16.

## Payload y alias

Los tensores se almacenan row-major, con la última dimensión contigua. Un tensor
propietario comienza exactamente después del anterior, sin huecos. Un alias
apunta a una entrada anterior propietaria, con forma, offset, tamaño y contenido
idénticos; no hay cadenas de alias y no se añaden sus bytes al payload.
El último dato propietario debe terminar exactamente al final del archivo.

Los límites de bytes y entradas se proporcionan al lector. Las herramientas
usan 16 MiB y 128 entradas como límites explícitos de validación del contenedor;
el modelo de aceptación tiene 52 entradas y aproximadamente 1 MiB. Un límite
insuficiente es un error, no permiso para omitir tensores.

La fixture [`tiny-package-v1.hex`](../tests/fixtures/tiny-package-v1.hex) tiene
176 bytes: cabecera, embedding 2x2, alias de salida y los bits de `[-0,1,-0.5,2]`.
Es una fixture del contenedor, no un stories260K válido. Permite comprobar el
formato entre Python y Rust sin descargar el modelo.

## Constantes auxiliares y fidelidad

El exportador compara los 47 tensores propietarios bit a bit entre `.pt` y
`.bin`. La salida comparte embedding y no añade otra matriz. Las antiguas tablas
RoPE del `.bin` se identifican como tales, no como pesos adicionales.

Para las 16 posiciones se usa la fórmula de `model.py` fijado en CPU F32:
frecuencias `1 / (10000 ** (arange(0,8,2)/8))`, producto exterior con posiciones,
y seno/coseno F32. Epsilon es el F32 más cercano a `1e-5`; la escala de atención
es `rsqrt(F32(8))`. La referencia matemática F64 y EML usarán esas mismas
constantes; sus diferencias respecto del recorrido F32 original se verifican
numéricamente en la etapa de referencia. No se promete igualdad de redondeo por
sustituir división por la raíz por multiplicación por esta escala.

`manifest.json` conserva hashes del paquete, cada tensor, inputs, información de
origen y exportador, junto con la política de constantes. Los hashes de todos
los pesos importados deben coincidir con sus tensores originales.
