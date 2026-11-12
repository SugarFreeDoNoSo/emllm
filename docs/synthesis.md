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
