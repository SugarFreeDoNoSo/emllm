# Design

## Context

La motivación y las decisiones de alcance están en [proposal.md](proposal.md). La base inspeccionada es `4e22ab502a46565966d0c0ce338f8cd9a22fb2ad`, en `develop`.

- `src/expr.rs` ofrece una arena append-only con pares internados y handles asociados a su identidad. No se puede clonar la arena ni trasladar un `NodeId` a otra por su índice.
- `src/program.rs` compila la clausura alcanzable mediante un recorrido iterativo izquierdo/derecho. La función privada `compile_nodes` ya abstrae la lectura de nodos, pero utiliza dos mapas hash de recorrido/remapeo y crea un snapshot propietario.
- `src/synth/scalar.rs` y `src/synth/model.rs` construyen únicamente estructura. Sus expansiones reciben concretamente `Arena`/`NodeId`; necesitan una frontera interna común para emitir también plantillas.
- `src/eval.rs` interpreta reales `f64` mediante exp/ln. `src/execution/` conserva ese kernel y reutiliza posiciones; ninguno constituye un ejecutor complejo ni un motor simbólico parametrizado.
- `src/model_io.rs`, `tools/import_model.py`, `tools/reference.py` y los corpus fijados ya aportan checkpoint, configuración y oráculos. `original_reference` instala hooks y copia trazas; no es directamente el camino que se debe temporizar como control PyTorch.
- Faltan el Transformer completo, tensores nativos y adaptadores temporizados. Las verificaciones existentes cubren escalares, operaciones y 901 composiciones, no el modelo entero.
- El intento de pesos compartidos alcanzó 10000000 nodos tras 106386 patrones completos, con RSS del proceso de medición de 1003044864 bytes. Fue un presupuesto de construcción, no una medición del límite físico del modelo completo.

`benchmark-tiny-model-eml` permanece en 13/29. Este cambio usa sus componentes implementados, pero no depende de dar por completadas sus tareas restantes ni modifica su interpretación temporal.

## Goals / Non-Goals

**Goals:**

- Separar una representación parametrizada de preparación y un resultado cerrado consumible por las operaciones EML existentes.
- Hacer que el trabajo estructural dependiente de tokens ocurra en la llamada nativa, manteniendo el modelo preparado independiente de ellos.
- Conservar dos fronteras observables: composición simbólica y lectura numérica externa. La salida principal es un vector lógico de expresiones, no un token decidido.
- Contrastar disposiciones reales no empaquetadas y emparejadas bajo las mismas fórmulas y orden de operaciones.

**Non-Goals:**

- Extender `Node` con variables o números, o sustituir el kernel real por logaritmos complejos.
- Convertir las expresiones de salida en decisiones de muestreo nativas, ejecutar generación incremental o prometer soporte para cualquier longitud.
- Introducir un formato persistente de modelos nativos, Redis, entrenamiento o destilación en este hito. La preparación se reutiliza en memoria durante la vida del proceso.

## Decisions

### 1. Modelo preparado independiente de los IDs, por longitud y disposicion

Se conserva el checkpoint original, los cinco bloques, las formas 64/172, cabezas Q/KV 8/4 y vocabulario 512. La preparación recibe paquete validado, longitud 8 o 16, disposición y límites; **no recibe los IDs de la solicitud**. La longitud permite preparar causalidad, RoPE y formas, sin especializar los valores de entrada.

```text
Checkpoint validado + T + disposicion
                  |
                  v
       Base One/Magma + plantilla
                  |
IDs --> simbolos nativos
                  |
                  v
      [ componer solicitud nueva ]
                  |
                  v
        Grafo cerrado + 512 raices
                  |
                  v
       Decodificar y verificar fuera
```

La base contiene los pesos, auxiliares, constantes de las expansiones y los 512 símbolos del vocabulario. Los pesos se sintetizan una vez por bits; los alias se conservan mediante mapas de raíces. Los símbolos tienen representaciones canónicas One/Magma de sus IDs y una asociación privada con su entrada del vocabulario. Reconocer un símbolo registrado usa identidad/metadata; no se evalúa su expresión para descubrir el índice.

La codificación externa solo valida y asocia IDs con esos símbolos, preservando posiciones. La selección de filas de embedding ocurre **dentro** de la composición. No se admite como entrada codificada una matriz de activaciones precalculadas.

Se elige una preparación por longitud frente a una plantilla universal de contexto 512, que sobredimensionaría este experimento. Longitudes distintas se rechazan explícitamente. Preparar otra longitud, disposición o versión tiene coste e identidad propios.

### 2. Plantilla de One/Magma con puertos formales confinados

La plantilla contiene referencias a la base, puertos de embedding y aplicaciones Magma en orden topológico. Hay `T*64` puertos lógicos para los canales de las posiciones de entrada. Son parámetros de una representación de composición, **no nodos añadidos al lenguaje cerrado**.

Las referencias de plantilla distinguen base, puerto e instrucción anterior con campos privados y procedencia. Se almacenan con índices de tamaño fijo y rangos disjuntos comprobados; no se guarda un `Arc` por arista. Los constructores validan hijos y límites antes de insertar. La plantilla publicada es inmutable y no implementa la frontera de programa cerrado.

El ensamblador del Transformer reduce RMSNorm, proyecciones, RoPE, atención, SwiGLU y residuales a esas aplicaciones antes de temporizar. Se conservan sumas balanceadas de pares adyacentes, el primer score como pivote de softmax y las expansiones verificadas en la base. El recorte estructural a las raíces requeridas de la plantilla pertenece a la preparación y no consulta valores de entrada.

Se rechaza preparar un DAG completo para cada prefijo y luego cronometrar la devolución de sus raíces: eso adelantaría el trabajo que este experimento pretende medir. También se evita mantener operadores tensoriales numéricos ocultos en la plantilla.

### 3. Expansiones compartidas mediante un constructor interno

Se extrae una interfaz privada de construcción estructural para One, Magma y obtención de constantes por bits, con referencias asociadas al constructor. Las APIs públicas actuales conservan sus firmas, resultados y errores; su implementación usa el adaptador de arena. El compilador de plantillas usa otro adaptador, que resuelve constantes en la base ya preparada.

Las fórmulas permanecen en una única implementación: duplicarlas en un segundo intérprete arriesgaría divergencias numéricas y de dominio. Las nuevas pruebas deben comprobar equivalencia de los grafos instanciados con la construcción cerrada existente, además de volver a ejecutar corpus y regresiones originales. El dominio de `Rational` y el kernel de `magma_value` permanecen como contratos de compatibilidad.

### 4. Base inmutable y delta privado por solicitud

Los tipos propuestos son `PreparedModel`, `NativePrefix`, `NativeOutput` y contextos privados de plantilla/solicitud, bajo `src/native/`. Son nombres de diseño, no APIs existentes.

- La base posee un `Program` de constantes, mapas de tensores/vocabulario y un índice inmutable de pares para compartir subexpresiones ya presentes.
- La preparación posee la plantilla y su metadata de formas/raíces. Las tablas de construcción anteriores se liberan cuando dejan de ser necesarias.
- Cada llamada a `compose` valida la entrada, crea un delta append-only y un índice de internado nuevo, selecciona los embeddings y asocia puertos con sus raíces de base.
- Una aplicación de plantilla resuelve sus operandos a referencias cerradas y reutiliza un par de la base/delta o añade uno al delta. El remapeo de instrucciones y toda esta búsqueda forman parte del tiempo nativo.
- El resultado retiene la base necesaria mediante propiedad compartida y posee sus pares y raíces. No necesita conservar la plantilla ni el índice mutable de construcción. La función libera sus temporales antes de devolverlo.

El grafo lógico concatena base y delta, con One único. Sus vistas devuelven exclusivamente `One` o `Magma` con hijos anteriores. Las referencias públicas llevan procedencia, y no se convierten handles de una arena en handles nativos usando índices desnudos. Un resultado puede ampliarse estructuralmente para usar sus raíces como operandos sin evaluar sus valores; esa ampliación tiene un contexto y coste propios.

Se prefiere este almacenamiento a copiar todo el modelo a una arena nueva por repetición. También evita una arena global creciente: reutilizar su internado entre repeticiones convertiría la segunda corrida en un acierto de caché de la salida.

### 5. Empaquetado de filas y canales, no nueva aritmetica compleja

Para pesos reales `W` de forma `n*m`, la vista emparejada es:

```text
C[i,j].re = W[2*i,j]
C[i,j].im = W[2*i+1,j]
```

Las activaciones `[T,n]` agrupan el último eje como `[T,ceil(n/2)]` pares. Se conserva `n` y se marca cualquier componente de relleno. Es una representación de dos expresiones reales; `Node::Magma` y el evaluador conservan el significado real.

Con entrada real `x`, cada par de `C*x` reúne dos salidas reales. Con entrada también empaquetada `z[j]=(x[2*j],x[2*j+1])`, una proyección real general exige:

```text
y[i].re = sum_j(W[2*i,2*j]   * z[j].re + W[2*i,2*j+1]   * z[j].im)
y[i].im = sum_j(W[2*i+1,2*j] * z[j].re + W[2*i+1,2*j+1] * z[j].im)
```

Estas fórmulas muestran las cuatro contribuciones, no una licencia para reagruparlas: se emiten productos en el orden original de canales y se aplica la misma reducción balanceada. No se reemplaza una matriz real arbitraria por una matriz compleja con la mitad de sus grados de libertad.

Reglas de operadores:

- RMSNorm suma cuadrados de todos los canales reales válidos y divide por la dimensión real original. El relleno no contribuye.
- SiLU y la puerta de SwiGLU actúan por componente; el producto de puertas no es una multiplicación compleja con términos cruzados.
- RoPE usa el par adyacente original y las expresiones `(a*c-b*s, a*s+b*c)`; los coeficientes siguen siendo constantes reales sintetizadas.
- Q/K/V conservan sus ejes de cabeza y posición. Los productos escalares de atención suman contribuciones reales; scores y probabilidades conservan su eje temporal real y su normalización causal. La vista de canales no convierte softmax en una función compleja.
- Embedding y salida son vistas del mismo conjunto de raíces base, incluso cuando una vista agrupa filas por tokens y otra agrupa canales de activación.

Las vistas y operadores empaquetados se ejercitan directamente. Cualquier desempaquetado estructural necesario dentro de una solicitud se cuenta en su tiempo y memoria. No se acepta una variante que solo renombre el resultado ya calculado por una corrida anterior.

Dos `f64` por elemento emparejado conservarían los mismos bytes brutos que dos elementos reales. En el núcleo simbólico cuentan handles, pares de nodos, plantillas e índices: ninguno recibe automáticamente un descuento del 50 %. La ausencia de mejora también es un resultado válido.

### 6. Lectura numerica externa y clausura validada

La salida nativa ya es un grafo cerrado; la lectura numérica es una operación posterior explícita. El adaptador de decodificación obtiene su clausura de 512 raíces y produce un `Program` validado para los evaluadores existentes. No añade un constructor público de programas desde pares arbitrarios.

La vista base+delta es densa y tiene un tamaño declarado. Para este camino se usará un recorrido iterativo con estados compactos y remapeo denso acotado, conservando visita izquierda/derecha, raíces repetidas, presupuesto alcanzable y validación de referencias/ciclos. Esto evita replicar dos mapas hash por nodo a escala del modelo. Su equivalencia con el compilador existente se comprueba sobre las mismas fixtures; la API de compilación anterior conserva sus contratos.

Compilar para decodificar, planificar, evaluar con ambos motores y comparar quedan fuera del tiempo nativo y se miden por separado. El `Program` numérico y el plan se descartan tras la validación de la muestra; no se conserva estado numérico para la siguiente composición.

Las salidas diagnósticas añaden raíces de etapas y tienen preparación e identidad separadas. Su metadata permite localizar bloque, etapa, posición y coordenada; un fallo del evaluador conserva además su nodo lógico. Las corridas temporizadas tienen únicamente las 512 raíces lógicas de logits.

### 7. Datos, tolerancias y prueba de independencia

Se reutilizan `tools/tiny-model.sources.json`, `tools/reference.sources.json`, el paquete canónico y las fixtures originales. El paquete observado tiene SHA-256 `434011a9e90db0bd15b39b80f11676c9b238b27ca8c526a9b0b87efc5b02ecd5`. El lector vuelve a validar formas y alias; los hashes se verifican desde herramientas antes de preparar.

Para demostrar reutilización se añade una variante de prueba por longitud: conservar todos los IDs salvo el último, que se sustituye por `(id+1) mod 512`. Se mantiene BOS, longitud y posiciones. Sus referencias se generan y fijan antes de la composición; son pruebas adicionales y no sustituyen las dos entradas del benchmark.

| Comparacion | Tolerancia por elemento |
| --- | --- |
| Constantes frente al F32 exacto elevado a F64 | `1e-9 + 1e-7*abs(ref)` |
| Operaciones/componentes frente a formulas F64 | `1e-6 + 1e-5*abs(ref)` |
| Etapas frente a la referencia explicita F64 | `1e-4 + 1e-4*abs(ref)` |
| 512 logits frente al original F32 | `1e-4 + 1e-4*abs(ref)` |
| Evaluadores EML entre si sobre el mismo programa | `1e-12 + 1e-12*abs(ref)` |
| Empaquetado frente a no empaquetado, mismo evaluador | `1e-12 + 1e-12*abs(ref)` |

Se conservan además las tolerancias originales 1e-9, 1e-8 y 1e-6. NaN, infinito, dominio inválido o desbordamiento invalidan el caso. Un mismo top-1 no sustituye estas comparaciones. El éxito de corpus aislados no acredita la composición completa.

### 8. Perfil inicial de recursos ampliado y configurable

La máquina inspeccionada declara 25769803776 bytes de RAM física, 24 GiB. Se propone el siguiente perfil inicial para las herramientas nuevas, seleccionable y registrado antes de cada corrida; los valores no son una predicción de consumo ni una garantía de cabida:

| Recurso | Limite inicial |
| --- | ---: |
| Paquete de entrada | 2097152 bytes, 64 tensores |
| Base de constantes, incluido One | 50000000 nodos |
| Plantilla, aplicaciones nuevas | 250000000 instrucciones |
| Puertos | 1024, correspondientes a 16*64 |
| Grafo cerrado residente, base + delta | 300000000 nodos |
| Raices, incluidas las diagnosticas | 1000000 |
| Compilacion/evaluacion externas | 300000000 nodos alcanzables |
| Plan numerico de validacion | 300000000 posiciones |
| RSS del proceso completo | 16 GiB |
| Tiempo de cada fase identificada | 1800 segundos |

La base y la plantilla se contabilizan separadamente del grafo cerrado de cada solicitud: las instrucciones de plantilla no son nodos adicionales de ese grafo, pero sí almacenamiento residente. El límite de nodos cerrados cuenta toda la base residente y el delta, aunque la decodificación alcance un subconjunto. El presupuesto de compilación numérica conserva su semántica de nodos alcanzables.

Se carga una combinación de longitud/disposición por proceso medido para evitar retener innecesariamente todas las plantillas. Las herramientas permiten elegir otro perfil finito antes de comenzar, registran su identidad y no lo amplían silenciosamente durante una corrida. Las constantes del benchmark anterior no se cambian.

El supervisor externo controla tiempo y observa RSS; la plataforma, intervalo y posibles sobrepasos entre muestras quedan declarados. El RSS máximo de proceso se contrasta también al terminar cuando el sistema lo proporciona. Una observación muestreada no se presenta como cota exacta ni como límite duro del sistema operativo. Si no se puede verificar un presupuesto exigido, su estado queda sin acreditar.

En vectores se reportan longitud, capacidad y tamaño real de elemento. Para los mapas de internado, `std` no expone todos los bytes de control/asignador: capacidad por tamaño de clave/valor se etiqueta como payload modelado, no como memoria total exacta. La porción no observable se declara y el RSS conserva su papel de medida del proceso.

Los hitos de recursos son base completa, plantilla, composición y validación completas. Cada uno conserva su evidencia; un fallo detiene sus dependientes y deja las tareas incumplidas, sin cambiar modelo ni tolerancias. Se permite gastar más que los controles para obtener la referencia inicial.

El supervisor reutilizable se implementa antes de esas corridas completas, con eventos mínimos de inicio/fin de fase. El benchmark final lo integra; los intentos intermedios ya disponen de observación de memoria, timeout y registro de terminaciones inesperadas.

### 9. Protocolo de benchmark y controles

Modos: `eml-native-real`, `eml-native-packed`, PyTorch original F32, llama2.c F32 y llama.cpp F32. Cada combinación modo-prefijo tiene proceso y configuración identificados, CPU/un hilo, tres calentamientos y diez repeticiones como mínimo. El modo nativo conserva base/plantilla/símbolos y crea un delta nuevo por llamada. Los controles conservan pesos, pero reinician activaciones y KV; el reinicio se informa aparte con política fija.

El cronómetro monotónico nativo envuelve `compose`: comienza antes de validaciones/asignaciones por solicitud y termina al recibir el grafo cerrado. Incluye búsqueda de embeddings, asociación, creación de nodos, empaquetados por solicitud y liberación de temporales internos. No incluye convertir el checkpoint, crear la plantilla, codificar los IDs, evaluar resultados ni emitir archivos/logs.

La validación de cada muestra ocurre después de detener su cronómetro, antes de descartarla. Se declara este intercalado en el reporte y se mantiene la misma política para las dos disposiciones. No se generan muestras cronometradas desde grafos previamente almacenados. Las métricas estructurales y la preparación también se reportan: `base_nodes`, instrucciones/puertos, nodos nuevos, clausura alcanzable, raíces, profundidad, componentes reales, capacidades, metadatos e índices auxiliares.

Los tiempos de preparación, codificación, composición, compilación de lectura, planificación numérica, cada evaluador, comparación, selección de token y emisión tienen nombres distintos. El padre mide además el recorrido extremo a extremo y declara descargas/instalación como costes externos. Un pico RSS global no se divide por fases mediante restas de máximos; snapshots de RSS o máximos muestreados se identifican como tales.

Los controles numéricos reutilizan datos y referencias, no sus hooks de trazado dentro del tiempo de forward:

- PyTorch usa el original fijado por `tools/reference.sources.json`, modo evaluación, CPU/un hilo y sin hooks. Las fórmulas explícitas F64 sirven de oráculo y no se presentan como el original con `.double()`.
- llama2.c usa la revisión `350e04fe35433e6d2941dce5a1f53308f87058eb`, checkpoint `.bin` verificado y un adaptador del forward real. Se declara su recorrido token por token y la proyección de logits en cada posición.
- llama.cpp usa una revisión inmutable registrada antes de adaptar y medir, conversión F32 verificada contra todos los tensores/alias/layouts e IDs inyectados sin retokenizar. GPU desactivada, hilos efectivos registrados y logits de la última posición.

El protocolo versionado emite identidad de motor/entrada, `output_kind` (`eml-expressions` o `numeric-logits`), fases, muestras, logits de validación y errores por etapa. El resumen separa la comparación de las dos disposiciones, que comparten frontera, del contraste con controles numéricos. No genera un ranking de aceleración numérica basado en omitir la lectura de expresiones. Con diez muestras, el p95 por rango más próximo es la máxima.

### 10. Organizacion y pruebas decisivas

```text
src/native/
  mod.rs          frontera publica y errores por fase
  prepared.rs     base, vocabulario y modelo preparado
  template.rs     constructor y plantilla parametrizada
  compose.rs      delta por solicitud e instanciacion
  codec.rs        codificacion de simbolos y validacion externa
  tensor.rs       formas y operaciones estructurales
  packed.rs       vistas y operaciones por canales emparejados
  model.rs        Transformer y raices diagnosticas
tools/
  native_resources.py    supervision compartida de fases y recursos
  verify_native_model.py
  benchmark_native.py
  adapters/       controles numericos y protocolo versionado
examples/
  infer_native.rs
docs/
  native-benchmark.md
```

La ubicación exacta de auxiliares privados puede ajustarse durante implementación sin cambiar la frontera. El runtime permanece `std`; dependencias numéricas y hashes de contenido se mantienen en herramientas externas.

Pruebas esenciales: puertos ajenos/no asociados; dos solicitudes de igual longitud con contenido distinto; repetición A/B/A; fallo entre solicitudes; salida viva tras liberar plantilla; consumo estructural posterior; ausencia de llamadas al evaluador en preparación/composición; límites exactos; alias; dimensiones impares; proyección real no compleja; SiLU/puertas por componentes; causalidad/GQA/RoPE; corpus existente; un bloque y cinco bloques con trazas; los 512 logits en todas las combinaciones; protocolo de tiempos, memoria, muestras inválidas y clasificación de trabajo diferido.

## Risks / Trade-offs

- [Plantilla, base y delta exceden memoria aun con perfil ampliado] -> Medir cada hito y todas las capacidades; usar base compartida y remapeos densos. El perfil es configurable, no una promesa de completar el modelo.
- [Un grafo simbolico correcto estructuralmente falla al evaluarse] -> Mantener la validación externa completa como aceptación, con las mismas tolerancias y evidencia de primer fallo.
- [La preparacion incorpora tokens o resultados por accidente] -> API de preparación sin IDs, fixtures de reutilización y trazabilidad separada de base, plantilla, entrada y salida.
- [Compartir estado de solicitudes falsea repeticiones] -> Identidades privadas, deltas e internado nuevos, pruebas A/B/A y de fallos intercalados.
- [Empaquetar introduce mezclas de componentes o ahorro ficticio] -> Fórmulas reales explícitas, casos no lineales/no complejos, dimensiones originales y contabilidad de ambas componentes.
- [Se confunde composicion con inferencia numerica terminada] -> Tipos de salida y fases obligatorios en JSON y resumen; selección de token solo en validación externa.
- [Factoring de sintesis o compilacion rompe contratos anteriores] -> Adaptadores internos, núcleo público conservado y verificación diferencial de fixtures, corpus y errores existentes.

## Migration Plan

La ampliación es opt-in. Se parte de las APIs verificadas, se añaden base/plantillas y composición pequeña, se integra el Transformer no empaquetado, después las vistas emparejadas y por último los controles y la medición completa. Cada paso tiene aceptación específica en `tasks.md`.

La reversión consiste en dejar de usar `native` y sus herramientas; no hay migración de archivos de usuario ni cambio de formato EMLTINY. El resultado de este cambio no completa automáticamente la lista anterior. El apply mantiene commits por tarea, revisión de la PR y autorización antes de merge/archivo.

## Open Questions

- La revisión exacta de llama.cpp y los flags disponibles se fijarán al construir su adaptador y quedarán en su manifiesto antes de medir. Esta elección no cambia el contrato CPU/F32/IDs ni el protocolo.
- El consumo real y la utilidad del empaquetado son resultados pendientes del experimento; no se presupone una mejora para aceptar el cambio.
