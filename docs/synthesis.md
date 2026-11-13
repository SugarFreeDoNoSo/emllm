# Síntesis constructiva One/Magma

Estas derivaciones definen las expansiones del núcleo. Todas las igualdades de
esta sección son sobre los reales; no prometen igualdad exacta en `f64`.
`M(x,y)` abrevia `Magma(x,y)` y `1` abrevia exclusivamente el nodo `One`.
Los restantes nombres son referencias a expresiones, nunca nuevas instrucciones
ni hojas numéricas. Ninguna construcción necesita evaluar sus argumentos.

## Exponencial y cero

Definimos:

```text
E(x) = M(x, 1)
e    = E(1)
z    = M(1, E(e))
```

Como `ln(1)=0`, `E(x)=exp(x)>0` para todo real x. Por tanto
`z=exp(1)-ln(exp(e))=e-e=0`. Los argumentos de logaritmo de estas
construcciones son 1 o exponenciales, siempre positivos.

El evaluador calcula la expresión de z tal cual, con la cancelación propia de
su precisión. No reconoce el nombre z ni lo sustituye por una constante cero.

## Logaritmo de una expresión positiva

Para y>0:

```text
L(y) = M(z, E(M(z, y)))
```

La expresión interior `M(z,y)` vale `1-ln(y)`. Su exponencial es positiva;
la expresión exterior vale `1-(1-ln(y))=ln(y)`. El único requisito adicional
es y>0; el segundo logaritmo recibe una exponencial positiva.

## Suma sin exigir signo positivo a sus operandos

Definimos el residuo positivo:

```text
P(x) = M(x, E(x)) = exp(x)-x
```

La desigualdad `exp(x)>=1+x` demuestra `P(x)>=1` para todo real x.
En consecuencia, `L(P(x))` siempre satisface su precondición.

La suma se construye así:

```text
C(x,y) = M(L(P(x)), E(y))
A(x,y) = M(x, E(C(x,y)))
```

Al sustituir las definiciones:

```text
C(x,y) = exp(ln(P(x))) - ln(exp(y)) = P(x)-y
A(x,y) = exp(x) - (P(x)-y)
       = exp(x) - (exp(x)-x-y)
       = x+y
```

Todos los logaritmos reciben P(x), que es al menos 1, o una exponencial.
Esta derivación admite operandos reales con cualquier signo.

## Negación y resta

```text
N(x)   = M(z, E(A(1,x)))
S(x,y) = A(x, N(y))
```

La primera expresión vale `1-(1+x)=-x`; la segunda vale `x-y`.
Los argumentos logarítmicos adicionales son exponenciales, siempre positivos.

## Producto

Para a>0 y b>0:

```text
Q(a,b) = E(A(L(a), L(b)))
```

Entonces `Q(a,b)=exp(ln(a)+ln(b))=a*b`. Para extenderlo a cualquier
signo usamos las descomposiciones `x=E(x)-P(x)` y `y=E(y)-P(y)`:

```text
u = Q(E(x), E(y))
v = Q(E(x), P(y))
w = Q(P(x), E(y))
t = Q(P(x), P(y))
F(x,y) = S(S(u,v), S(w,t))
```

Todos los argumentos de Q son estrictamente positivos por las demostraciones
anteriores. La expansión algebraica verifica:

```text
u-v = E(x) * (E(y)-P(y)) = E(x)*y
w-t = P(x) * (E(y)-P(y)) = P(x)*y
F(x,y) = (E(x)-P(x))*y = x*y
```

Se usa esta agrupación de restas para evitar sumar primero los cuatro términos
positivos de magnitud potencialmente grande. La identidad no garantiza que
los intermedios quepan en `f64` ni que la cancelación sea pequeña.

## Racionales normalizados del dominio inicial

La frontera de entrada recibe enteros p y q, rechaza q=0 y normaliza p/q
mediante su máximo común divisor, con q positivo. El dominio inicial acepta
los valores normalizados con `abs(p)<=2` y `1<=q<=2`, incluyendo todas las
combinaciones mínimas exigidas por las especificaciones.

- Cero se construye como z.
- Los enteros positivos 1 y 2 se construyen desde One mediante suma de unidades.
- Para una fracción propia normalizada con q=2, necesariamente `abs(p)=1`.
  Su magnitud se construye como `E(N(L(A(1,1))))`.
- El signo negativo se construye aplicando N a la magnitud.

Para la fracción, `A(1,1)=2>0` y
`E(N(L(2)))=exp(-ln(2))=1/2`; todos sus logaritmos tienen dominio positivo.
Los metadatos enteros seleccionan una construcción finita; no se convierten
en instrucciones ejecutables ni se entregan al evaluador como valores enlazados.

## Alcance numérico

Las identidades anteriores son constructivas y se justifican algebraicamente,
sin deducirlas de coincidencias en muestras. Las comprobaciones nativas deben
evaluar los DAGs expandidos y compararlos con una referencia independiente.

Los exponentes anidados pueden desbordar aun cuando el resultado real final sea
pequeño. La cancelación y el subdesbordamiento pueden alterar incluso el dominio
de un logaritmo posterior. El evaluador informa esos fallos; no usa clamps,
hojas numéricas, reconocimiento de fórmulas ni sustituciones por la referencia.

El dominio de aceptación numérica incluye los racionales iniciales, suma,
negación y producto sobre {-1/2,0,1/2}, y las tres fixtures neuronales de la
especificación. Sus tolerancias deben comprobarse; no se extrapolan al conjunto
de todos los DAGs o a redes de mayor tamaño.

## Resultados nativos iniciales

Medición realizada sobre la implementación de la tarea 3.3 (commit `6e2639e`),
con `rustc 1.92.0 (ded5c06cf 2025-12-08)`, LLVM 21.1.3, host
`aarch64-apple-darwin`, perfil de pruebas sin optimización. Comando:

```sh
cargo test --test synthesis -- --nocapture
```

| Casos | Error absoluto máximo observado | Tolerancia exigida | Máximo de nodos alcanzables por programa |
| --- | --- | --- | --- |
| Diez pares p/q con -2<=p<=2 y 1<=q<=2 | 5.551e-17 | 1e-9 | 27 |
| Nueve pares x,y en {-1/2,0,1/2}, cinco salidas por par | 2.220e-14 | 1e-8 | 203 |

Las cinco salidas son x+y, x*y, -x, x*y-x y x*(x+y). Cada programa se
inspecciona para verificar que contiene exclusivamente One/Magma y que sus
dependencias preceden a sus usos. La referencia calcula esas cinco expresiones
convencionalmente dentro de las pruebas, sin llamar a los constructores EML.

También pasan las pruebas de denominador cero, normalización con extremos
enteros y presupuesto de síntesis agotado. Las pruebas de evaluación cubren
argumentos logarítmicos no positivos, desbordamiento, no finitud y límites.
Estos fallos se devuelven como errores identificables, no como resultados de la
referencia. Las únicas llamadas trascendentes de producción están en
`src/eval.rs`; `Node` conserva únicamente One y Magma.

Un fallo en los casos de aceptación bloquea la integración de la red y obliga
a revisar las expansiones o la decisión numérica. No se añaden primitivas,
clamps ni sustituciones para conseguir que una prueba pase. Los números
anteriores son observaciones locales reproducibles mediante el comando, no
cotas globales de error, tiempo o memoria.

## Puerta de viabilidad de stories260K: identidades candidatas (4.1)

Esta sección conserva `M(x,y)=exp(x)-ln(y)`. Los metadatos IEEE usan potencias
de dos, pero no añaden `exp2`, `log2` ni hojas numéricas al lenguaje.

### Decodificación diádica de F32

Para bits de signo s, exponente almacenado b y fracción f (23 bits):

- b=255 se rechaza antes de construir: NaN/infinito.
- b=0: magnitud `f * 2^-149`, incluidos los dos ceros si f=0.
- 1<=b<=254: magnitud `(2^23+f) * 2^(b-150)`.
- El signo es `(-1)^s`. Desplazar los ceros finales de la mantisa y aumentar
  el exponente conserva exactamente el racional diádico y acorta el trabajo.

El signo de cero permanece en los metadatos importados; ambos ceros construyen
la expresión z. No se promete que la cancelación del DAG preserve un cero IEEE
negativo: la tolerancia de síntesis compara valores, no bits de la evaluación.

### Mantisa fraccionaria y escala positiva

Sea `l=L(A(1,1))=ln(2)` y `h=E(N(l))=1/2`. Definimos, para x>0:

```text
H(x) = E(A(N(l), L(x))) = x/2
D(x) = E(A(l, L(x)))    = 2*x
```

La orientación de A es deliberada: su primer operando es siempre ±ln(2),
no `ln(x)`. Así su residuo P es fijo y pequeño; invertir los argumentos podría
volver a introducir `exp(exp(ln(x)))`. Los dominios reales son x>0 y las
exponenciales adicionales positivas. No se evalúa x para escoger orientación.

Para mantisa entera no nula m con k bits, se construye `m/2^k` recorriendo los
bits de menor a mayor: empezar en h en el primer bit 1; para cada bit siguiente,
aplicar H a x si es 0 y a `A(1,x)` si es 1. Por inducción el acumulador es la
fracción binaria de los bits ya leídos, estrictamente entre 0 y 1; al finalizar
está en `[1/2,1)`. Una mantisa de 24 bits no necesita un entero de magnitud 2^24.
Para `m*2^e`, aplicar D `e+k` veces si es positivo, o H `-(e+k)` veces si es
negativo. El signo final se aplica con N, y cero con z.

La metadata controla únicamente un número finito de expansiones y bits de
mantisa. Todos los logaritmos de H/D reciben expresiones positivas en reales;
las sumas de mantisa tienen primer operando One. La negativización final N
todavía usa exponenciales de la magnitud: no se afirma soporte numérico de todos
los F32 negativos. La implementación debe verificar **cada** escalar del corpus,
incluidos sus extremos documentados, y detenerse ante cualquier fallo requerido.

### Operaciones candidatas iniciales del modelo (histórico 4.1)

Se propusieron como candidatos iniciales A, N, S y F de las derivaciones anteriores.
La implementación revisada de 4.3 se deriva al final de esta guía.
Para las operaciones restantes:

| Operación | Expansión | Dominio real adicional |
| --- | --- | --- |
| Exponencial | `E(x)` | ninguno |
| Recíproco | `R(x)=E(N(L(x)))` | x>0 |
| Raíz inversa | `U(x)=E(F(-1/2,L(x)))` | x>0 |
| SiLU | `F(x,R(A(1,E(N(x)))))` | denominador `1+exp(-x)>0` |
| Producto escalar | suma de `F(xi,yi)` | ninguno |
| RMSNorm | `F(F(xi,U(A(mean(F(xj,xj)),eps))),wi)` | eps>0 |

`-1/2`, epsilon, escala de atención y coeficientes RoPE deben ser expresiones
sintetizadas. Dividir por una longitud usa su recíproco positivo sintetizado;
la metadata de forma determina las iteraciones, nunca una operación numérica
del evaluador. Las sumas/reducciones agrupan pares adyacentes en árbol; una cola
impar pasa al nivel siguiente. Este es el orden del oráculo del corpus, distinto
del orden interno que el backend matricial de PyTorch pueda elegir.

Softmax para la fila causal t construye únicamente scores `s0..st`, elige
el **primer score simbólico** p=s0 y forma `ei=E(S(si,p))`; las probabilidades
son `F(ei,R(sum(e)))`. Restar un mismo pivote se cancela en numerador/denominador
sobre los reales. La suma es positiva en reales; no requiere comparación nativa,
máximo de referencia ni máscara infinita. No controla el rango de `si-p` como
lo haría un pivote máximo. Su finitud y error deben demostrarse con el corpus.

Estos dominios son algebraicos. A y F pueden desbordar, sufrir cancelación o
cambiar el signo calculado de un argumento positivo. R/U/SiLU heredan esos
riesgos. La puerta 4.2–4.4 exige los thresholds fijados, pureza y presupuestos;
disponer de estas identidades no la da por superada ni permite alimentar el DAG
con activaciones de referencia.

### Contraejemplo ejecutado: A(7,0)

```sh
cargo run --release --example synthesis_probe -- --dump
```

El ejemplo construye 7 desde One mediante seis `A(One,acumulador)` y construye z;
compila todos los programas antes de evaluar. Sus entradas evaluadas son `[7,0]`.
El programa de `A(7,z)` contiene V/E/R/D=`40/78/1/37`; el plan necesita cuatro
posiciones. Ambos evaluadores fallan con `ExponentialOverflow`, nodo lógico
**38**, `Magma(37,0)`, antes de entregar la raíz 39.

El nodo 37 representa `P(7)-0 = exp(7)-7 ≈1089.633158428459`, por lo que el
nodo 38 intenta su exponencial; el límite finito de exponente F64 es
aproximadamente 709.7827. `--dump` publica todos los nodos/pares para inspección.
Es evidencia nativa con Rust 1.92.0, `aarch64-apple-darwin`, release, y demuestra
el fallo del constructor histórico aun cuando el resultado real final sea 7.
No demuestra imposibilidad de otra expansión One/Magma: obliga a comprobar
las candidatas nuevas antes de ensamblar el modelo.

### API y verificación escalar

`synth::scalar::decode(F32Bits) -> Dyadic` extrae signo, mantisa impar y exponente
con operaciones enteras. `synth::scalar::synthesize(&mut Arena, F32Bits)` devuelve
un `NodeId` de esa arena o `BuildError`; no acepta activaciones, valores F64 ni
resultados de un evaluador. `F32Bits::new(u32)` rechaza NaN/infinito. El algoritmo
implementa la mantisa fraccionaria y H/D anteriores; no cambia `Rational`.

```sh
cargo test --release --test model_synthesis
target/tiny-model/venv/bin/python -B tools/verify_scalar_corpus.py
# Caso individual por bits, para reproducir un diagnóstico:
cargo run --release --example verify_scalars -- --bits 3727c5ac
```

La herramienta externa valida hashes y procedencia del corpus antes de ejecutar
el ejemplo Rust. Cada escalar se construye en una arena nueva, se compila como
programa cerrado y se evalúa con ambos recorridos, sin reutilizar resultados.
Límites explícitos por caso: 10000 nodos en construcción/compilación/planificación/
evaluación, 10000 posiciones, corpus de hasta 4 MiB. Se inspeccionan todos los
nodos, y las salidas se comparan con el F32 exacto elevado a F64 usando
`1e-9 + 1e-7*abs(ref)` y entre evaluadores con `1e-12 + 1e-12*abs(ref)`.

El primer fallo conserva bits, índice del corpus, procedencia tensor/índice,
etapa y diagnóstico lógico cuando existe. No se excluye ni se sustituye; los
casos posteriores se declaran no verificados. El reporte y logs quedan bajo
`target/analysis/scalar-synthesis/`. Las fixtures locales comprueban metadatos,
signos, ceros, magnitudes pequeñas/grandes, límites y procedencia; su éxito no
reemplaza la comprobación de los 280688 valores requeridos.

### Dominio escalar efectivamente verificado (4.2)

La puerta completa pasó los **280688 valores distintos** del corpus fijado
`d4c9207a3ea4e9b526bdc4c6b2a791cd6439586c853a8b58e6c3f9c2095677c5`, con ambos
evaluadores y todos los límites anteriores. Máximos por programa: V=829,
D=823, ocho posiciones numéricas. La mayor fracción del umbral escalar fue
`3.1264782e-7` (muy inferior a 1). El máximo error absoluto fue `0.06689453125`
en un corpus que incluye magnitudes de billones; no representa el error de
las constantes pequeñas. Duración observada del recorrido completo: 12.685 s,
sin carga de fuentes ni compilación Rust dentro de ese intervalo.

La evidencia cubre los valores concretos del checkpoint, constantes y corpus,
no todos los valores del intervalo entre sus extremos ni todos los F32 finitos.
Tampoco garantiza composiciones: una suma/producto de escalares correctamente
sintetizados aún puede fallar. La siguiente puerta debe verificar esas operaciones.
`target/analysis/scalar-synthesis/report.json` conserva comandos, límites,
revisión, hashes de fuentes/corpus, toolchain y contadores. Las 55 pruebas Rust
y 20 doctests pasan, incluidos los contratos y tolerancias originales.

## Bloqueo histórico de operaciones compuestas (4.3 inicial)

En `3a277e0`, `synth::model` incorporaba los candidatos A/E/F/N/S y las
composiciones R/U/SiLU anteriores. Esa validación numérica **no pasó**.

```sh
target/tiny-model/venv/bin/python -B tools/verify_operation_corpus.py
# Reproducción mínima, sin checkpoint ni Python:
cargo run --release --example verify_operations -- tests/fixtures/tiny-operation-blocker.txt
```

En esa revisión ambos comandos terminaron con código 1. El primer fallo fue el caso de
índice 10 (base cero), después de diez casos satisfactorios:

- Familia: `prefix8/explicit_f32/layers.0.attention_norm.squares[4]`.
- Ambos operandos: `embedding[301]` (posición 4, coordenada 45), bits
  **`0x3f8afc5f`**, valor F32 exacto **1.0858267545700073**.
- Producto F64 esperado: **1.179019740940035**.
- Programa One/Magma: V=356, E=710, R=1, D=301, diez posiciones.
- Ambos evaluadores: **`ExponentialOverflow` en el nodo lógico 312**, expresión
  `Magma(311,0)`. No se entregan resultados parciales.
- Quedan 6588 de las 6599 operaciones sin verificar. Ninguna reducción completa
  ni componente de la puerta 4.4 ha sido aceptado mediante estas expansiones.

Los límites eran 100000 nodos por etapa y 100000 posiciones; no se agotaron.
La carga del protocolo de desarrollo se acota a 2 MiB. El archivo `EMLOPS1`
contiene nombre de operación, bits F32 de hasta dos operandos y oráculo F64;
solo los bits entran a la síntesis y el oráculo se usa después de evaluar.
`target/analysis/operation-synthesis/` conserva corpus completo de operaciones,
caso mínimo, salida, categoría, procedencia, hashes, comando, toolchain y revisión.

Además, la prueba local
`candidate_reciprocal_root_and_silu_match_small_independent_cases` falló para
x=1.5 en la composición de SiLU: overflow en nodo 210, raíces `[58,149,258]`.
Las pruebas de dominio/procedencia/presupuesto pasaron. Las 18 pruebas originales
de síntesis/red y del plan pasaron con sus tolerancias; la suite nueva quedó
**fallida** en esa revisión.

El fallo demuestra que la expansión candidata del producto no es numéricamente
apta ni siquiera para este cuadrado moderado; no demuestra imposibilidad de
otra expansión One/Magma. El primer término positivo del producto era
`u=exp(x)^2≈8.77278`; la primera resta intentaba `exp(exp(u)-u+v)` con
exponente ≈6452.866. La revisión siguiente evita ese intermedio.

## Expansiones desplazadas y normalizadas verificadas (4.3)

Esta sección registra la revisión **0f576de**. La recuperación de escala y las
operaciones positivas vigentes se derivan en la revisión 4.4 al final.
En 0f576de, `synth::model` mantuvo E y usó las siguientes identidades. A, L y z conservan
sus definiciones originales; solo se invoca A con operandos acotados y una
orientación fija. Todas las constantes se sintetizan desde One/Magma.

### Reflexiones, suma y resta

```text
R_c(x) = M(L(c), E(x)) = c-x                     (c>0)
N_m(x) = R_1(R_2(R_1(x))) = -x
T(x)   = R_128(N_m(x)) = 128+x
A_m(x,y) = M(L(T(x)), E(R_128(y))) = x+y         (x>-128)
S_m(x,y) = A_m(x,N_m(y)) = x-y                   (x>-128)
```

Las tres reflexiones de N_m producen `1-x`, `1+x`, `-x`. La suma resta
`128-y` a `128+x`. El desplazamiento positivo permite L(T(x)) sin
descomponer x en diferencias de exponenciales. En F64 todavía se requieren
exponenciales finitas/no nulas de x, 1±x y 128-y; estas son condiciones
numéricas adicionales, no garantías derivadas únicamente de x>-128.

### Escala firmada y producto

Para s>0 y x>-128:

```text
B_s(x) = M(A(L(s),L(T(x))), E(128*s)) = s*x
u = B_(1/64)(x); v = B_(1/64)(y)
q = M(A(L(A(1,u)),L(A(1,v))), E(A(1,A(u,v))))
  = (1+u)*(1+v) - (1+u+v) = u*v
F_m(x,y) = doce aplicaciones q <- A_m(q,q) = x*y
```

El primer argumento de A en B_s es siempre L(s), fijado antes de construir.
Las parejas de constantes `(s,128*s)` son `(1/64,2)` y `(1/2,64)`;
sus bits F32 son `0x3c800000/0x40000000` y `0x3f000000/0x42800000`.
El centro 128 tiene bits `0x43000000`. No se evalúa ningún operando para
elegir un centro, una escala o el número de duplicaciones.

Un dominio real suficiente propuesto para F_m es `|x|,|y|<=32` y `x*y>-256`:
u,v están en [-1/2,1/2], por lo que 1+u y 1+v son positivos. Cada entrada a
las doce duplicaciones negativas es mayor que -128; las positivas no tienen
esa restricción real. Las cancelaciones en q y las duplicaciones amplifican
el error; el dominio no promete una cota F64 uniforme. La evidencia siguiente
cubre los casos concretos fijados, además de las regresiones locales de signos,
cancelación, `7+0`, `32*32`, `-32*7` y el cuadrado que fallaba.

### Recíproco, raíz inversa y SiLU

```text
R_m(x) = E(N_m(L(x))) = 1/x                     (x>0)
U_m(x) = E(N_m(B_(1/2)(L(x)))) = 1/sqrt(x)      (x>0, ln(x)>-128)
d = M(N_m(x),E(-1)) = exp(-x)+1
SiLU_m(x) = F_m(x,R_m(d))
```

La raíz inversa usa B_(1/2) en lugar del producto general. SiLU construye
directamente su denominador mediante Magma; -1 es un racional sintetizado.
La composición SiLU debe satisfacer además el dominio de F_m y las condiciones
numéricas de cada subexpresión. Ninguna de estas fórmulas usa clamps, hojas
numéricas ni cambios del kernel. Los errores de dominio, overflow, presupuesto
y procedencia siguen propagándose.

### Cobertura y límites comprobados

```sh
cargo test --release --test model_synthesis
cargo run --release --example verify_operations -- tests/fixtures/tiny-operation-blocker.txt
target/tiny-model/venv/bin/python -B tools/verify_operation_corpus.py \
  --output-dir target/analysis/operation-synthesis-revised
```

Los tres comandos pasan. Se verifican **6599/6599 operaciones** con ambos
evaluadores y `1e-6 + 1e-5*abs(ref)`, sobre el corpus original de SHA-256
`d4c9207a3ea4e9b526bdc4c6b2a791cd6439586c853a8b58e6c3f9c2095677c5`:

| Operación | Casos | Rango de operandos observado (aproximado) |
| --- | ---: | --- |
| Producto | 4314 | [-21.922198, 28.029603] |
| Suma | 567 | [-26.296731, 28.456139] |
| Resta | 1275 | [-34.019066, 26.285544] |
| Negación | 70 | [-4.095533, 4.215379] |
| Recíproco | 60 | [1.014766, 61.071312] |
| Raíz inversa | 103 | [0.03967469, 5.433550] |
| Exponencial | 140 | [-28.472718, 28.653899] |
| SiLU | 70 | [-4.095533, 4.215379] |

Productos esperados entre -117.218386 y 74.702185. Máximos por programa:
V=1013, E=2024, R=1, D=569 y 17 posiciones; estos máximos pueden corresponder
a casos distintos. Mayor fracción del umbral `0.00025848092631870485`,
diferencia entre evaluadores cero. Máximo error absoluto `0.05908203125`, en
un corpus con exponenciales de hasta `2.7811850760220205e12`; no es una cota
para las operaciones pequeñas. Recorrido observado: 1.325 s, excluyendo
verificación de fuentes y compilación Rust.

Se conservan los límites de 100000 nodos por etapa, 100000 posiciones y 2 MiB
de protocolo, además de la inspección One/Magma y los oráculos externos.
La evidencia revisada y hashes de código están en
`target/analysis/operation-synthesis-revised/report.json`; el fallo anterior
se conserva en `target/analysis/operation-synthesis/`. La fixture histórica
ahora pasa con V/E/R/D=601/1200/1/443 y 13 posiciones.

Las 58 pruebas Rust y 20 doctests pasan; Clippy sin advertencias. Las tolerancias
originales siguen vigentes. Pasar estas operaciones aisladas no demuestra las
reducciones completas, RMSNorm, softmax ni SwiGLU: requieren la puerta 4.4.

La primera ejecución de esa puerta verifica las 631 reducciones, pero falla
en un softmax cuyo término exponencial alcanza ≈3287 y vuelve a entrar a E
dentro de la suma desplazada. La reproducción, cobertura pendiente y costes
están en [la puerta de composiciones](tiny-model.md#puerta-de-composiciones-bloqueo-en-44).

## Revisión numérica de composiciones (4.4)

### Recuperación del producto con una escala fija

La construcción vigente conserva u=x/64, v=y/64 y los factores positivos
a=1+u, b=1+v. Reagrupa el offset como `A(a,v)=1+u+v` y forma:

```text
q = M(A(L(a),L(b)), E(A(a,v))) = u*v
T_c(q) = R_c(N_m(q)) = c+q
F_m(x,y) = M(A_m(L(4096),L(T_(1/8)(q))), E(512))
         = 4096*(q+1/8)-512 = x*y
```

Esto sustituye las doce duplicaciones. Para el dominio suficiente anterior,
`|x|,|y|<=32`, `x*y>-256`, tenemos `q>-1/16`, `q<=1/4`; por tanto
`T_(1/8)(q)` está entre 1/16 y 3/8 y su logaritmo tiene dominio positivo.
La exponencial de salida interna vale entre 256 y 1536; su **exponente** está
entre ln(256) y ln(1536). El otro término usa E(512), finito en F64. Se usa
A_m para sumar los logaritmos: usar A con primer argumento ln(4096) volvería
a crear una exponencial anidada demasiado grande.

Las constantes 1/8, 4096 y 512 tienen bits `0x3e000000`, `0x45800000` y
`0x44000000`; se sintetizan en la arena. Esta identidad sigue teniendo
cancelación y necesita validación, no promete error universal. Las **6599
operaciones** vuelven a pasar, con V máximo 910, D máximo 441 y 19 posiciones;
fracción máxima del umbral `2.176523734359192e-5`, diferencial cero.
Evidencia: `target/analysis/operation-synthesis-rescaled/report.json`.

### Suma y cociente positivos en dominio logarítmico

Para x,y>0:

```text
J(t) = M(t,E(-1)) = exp(t)+1
A_+(x,y) = E(A_m(L(x),L(J(S_m(L(y),L(x))))))
         = exp(ln(x)+ln(1+exp(ln(y)-ln(x)))) = x+y
D_+(x,y) = E(S_m(L(x),L(y))) = x/y
```

`synth::model::add_positive` y `divide_positive` implementan estas fórmulas.
Un dominio real suficiente para A_+ añade `ln(x),ln(y)>-128`, debido a las
sumas desplazadas. D_+ requiere `ln(x)>-128`. En F64 también deben caber
logaritmos, diferencias y exponenciales intermedias. J(t) suma 1 directamente
mediante Magma: no calcula `exp(exp(t))`. SiLU reutiliza esta misma J.

Softmax conserva `ei=E(S_m(si,s0))`, el pivote **s0** y el árbol de pares
adyacentes; solo cambia las expansiones de suma positiva y división:
`pi=D_+(ei, sum_+(e))`. Numerador y denominador permanecen conectados en un
único DAG. No hay máximo nativo, elección de fórmula evaluando operandos ni
normalización con activaciones externas.

En las 160 filas fijadas, los logaritmos de los operandos de las sumas
positivas del oráculo están entre -19.779728 y 16.801812; sus diferencias
entre -20.511209 y 11.662594. El denominador máximo es ≈19813436.74. Son rangos
observados, no límites universales. Las regresiones locales también normalizan
pares con magnitudes desde ≈2.1e-23 hasta ≈2.8e12 y comprueban dominio y
procedencia con ambos evaluadores.

### Resultado y límite de la puerta

Pasan **901/901 composiciones**: 631 productos escalares, 48 RMSNorm, 160
softmax y 62 filas SwiGLU. Fracción máxima del umbral `0.0020050764041026478`,
diferencial cero. La fixture histórica de softmax pasa con V/E/R/D
2157/4312/8/356 y 24 posiciones. Las 60 pruebas Rust, 20 doctests y tres pruebas
del ejemplo de componentes pasan; se conservan todas las tolerancias.

La **4.4 sigue pendiente por recursos**: sintetizar los pesos compartidos
agota 10000000 nodos tras 106386 de los 259577 patrones distintos, antes de
ensamblar el modelo. La síntesis individual sigue siendo correcta; el fallo
es de presupuesto de construcción. Medición, reproducción y estimación con
costes marginales en [la revisión de la puerta](tiny-model.md#revision-de-la-puerta-numerica-completa-y-bloqueo-de-recursos).
