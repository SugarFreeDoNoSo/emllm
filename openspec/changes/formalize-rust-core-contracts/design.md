# Design

## Context

Véase `proposal.md` para la motivación. El repositorio contiene README, documentación conceptual en `docs/report/emllm.tex`, un sitio estático y workflows documentales. No hay Cargo, fuentes Rust ni pruebas ejecutables del motor. El único cambio previo, `implement-one-magma-eml`, define el núcleo, sus dos capacidades y la red 2-2-1, pero tiene todas sus tareas pendientes.

Esta propuesta depende de esa implementación y concreta contratos adicionales; no trata sus capacidades planificadas como comportamiento ya disponible. Los contratos nuevos están en `specs/eml-program-contracts/spec.md` y `specs/eml-complexity-reporting/spec.md`. Las decisiones de API siguientes son propuestas, no nombres encontrados en código existente.

## Goals / Non-Goals

**Goals:**

- Reducir las obligaciones que cada consumidor debe comprobar mediante tipos y fronteras de validación.
- Separar responsabilidades por invariantes y motivos de cambio, conservando decisiones numéricas necesarias.
- Producir evidencia estructural y de código cuya interpretación sea reproducible y limitada a su alcance.

**Non-Goals:**

- Imponer un paradigma único, convertir cada módulo en un `trait` o exigir una complejidad ciclomática máxima.
- Certificar la validez numérica de cualquier DAG mediante su tipo estructural ni implementar un sistema de pruebas simbólicas de positividad.
- Incorporar persistencia de programas, ABI estable, identificadores entre procesos o un sistema propio de análisis de Rust.

## Decisions

### 1. Una frontera de tipos entre construccion y ejecucion

Proponer `Arena`, `NodeId` y `CompiledProgram` como nombres de referencia. `Arena` admite construcción incremental; `CompiledProgram` solo se obtiene mediante compilación y conserva campos privados. La API de evaluación recibe una referencia al programa y configuración explícita. No recibe la arena ni acepta raíces arbitrarias adicionales.

```text
Datos de frontera
       |
       v
Red / sintesis --> Arena + raices
                         |
                         v
                 Compilacion estructural
                         |
                +--------+--------+
                |                 |
                v                 v
       CompiledProgram       Error estructural
                |
                +--> Inspeccion y estadisticas
                |
                v
       Evaluacion + presupuesto
                |
          +-----+-----+
          v           v
       Salidas    Error numerico
```

La compilación valida pertenencia, raíces, resolución de referencias, ausencia de ciclos y presupuesto de nodos alcanzables; produce un orden topológico mediante recorrido iterativo. El orden es determinista para una misma estructura y lista de raíces, visitando operandos en su orden declarado. La lista de raíces debe ser no vacía; conserva orden y duplicados. Esta regla sobre raíces vacías es una decisión de diseño adicional a la base.

El programa guarda únicamente la clausura alcanzable de esas raíces, con referencias internas remapeadas. La evaluación se apoya en esta validación sin volver a recorrer el grafo para comprobar ciclos o procedencia en cada llamada. Se mantienen los accesos seguros de Rust: el objetivo no requiere eliminar comprobaciones automáticas de índices ni introducir `unsafe`.

**Alternativa descartada:** un indicador público `validated: bool` junto a nodos mutables. Permitiría separar el indicador del estado real y obligaría a revalidar constantemente. Un framework genérico de estados tipados también sería posible, pero dos tipos concretos expresan la frontera actual con menor coste de comprensión.

### 2. Referencias opacas y snapshot propietario

`NodeId` encapsula posición e identidad de arena. La identidad se representa mediante un token privado compartido por la arena y sus referencias; conservarlo vivo evita confundir una arena nueva con una destruida. No basta con un índice, ni con asumir que dos referencias con el mismo lifetime pertenecen a la misma arena. Los constructores y la compilación validan procedencia antes de usar posiciones locales.

La arena es append-only: nodos existentes no se modifican ni sus posiciones se reutilizan. No se ofrece clonación de una arena mutable que copie su identidad a otra arena independiente. El internado compara estructura y orden de operandos una vez validada su procedencia; nunca compara resultados numéricos.

`CompiledProgram` posee un snapshot compacto de sus nodos alcanzables, raíces y orden. No depende del lifetime de la arena. Puede compartirse inmutablemente; su inspección expone vistas de solo lectura, no acceso mutable ni constructores públicos sin validación. Los identificadores publicados para diagnósticos corresponden a nodos de ese snapshot y se resuelven mediante su inspección; no son índices persistentes ni intercambiables con los handles de construcción.

Esto permite seguir construyendo en la arena y compilar otra entrada sin alterar programas anteriores. Copiar el snapshot implica un coste lineal y puede duplicar almacenamiento entre programas; queda explícito, sin prometer compartición entre snapshots.

**Alternativas descartadas:** handles numéricos desnudos por falta de procedencia; un préstamo permanente de la arena por impedir liberarla y dificultar construcción posterior; referencias globales o contadores mutables globales para obtener identidad. Una arena con marcas generativas en tipos puede estudiarse más adelante si se justifica su complejidad de API.

### 3. Modulos por responsabilidad y extensibilidad localizada

Usar los nombres coherentes que establezca la implementación base. Si todavía no los define, la organización de referencia es:

| Módulo previsto | Responsabilidad y contrato | Dependencias internas permitidas |
| --- | --- | --- |
| `src/expr.rs` | Arena, nodos cerrados y procedencia de referencias | Biblioteca estándar |
| `src/synth.rs` | Racionales normalizados y expansiones algebraicas | Expresiones |
| `src/program.rs` | Compilación, programa validado e inspección estructural | Expresiones |
| `src/eval.rs` | Ejecución numérica, presupuesto y errores de evaluación | Vista del programa validado |
| `src/network.rs` | Composición de la red y validación de dimensiones | Síntesis y expresiones |
| `src/lib.rs` y ejemplo | Fachada pública y coordinación explícita de etapas | Módulos anteriores según la operación |

Los módulos inferiores no importan la red. Síntesis y compilación no llaman al evaluador. La referencia aritmética independiente permanece en pruebas/demostración y no se convierte en una ruta de ejecución de producción.

Los nodos se representan con un `enum` cerrado One/Magma y `match` exhaustivo. Las expansiones son funciones que componen nodos; los métodos de la arena protegen sus invariantes. La mutabilidad local del constructor y de los buffers de evaluación no requiere estado global, configuración ambiental ni efectos de entrada/salida dentro del núcleo.

Aplicación concreta de SOLID que documentará `docs/architecture.md`:

| Principio | Criterio aplicado |
| --- | --- |
| SRP | Agrupar decisiones que preservan un mismo contrato; separar semántica numérica, representación, síntesis y composición neuronal. Una función con muchas comprobaciones puede seguir teniendo una responsabilidad. |
| OCP | Extender constructores mediante nuevas composiciones sin ampliar las dos formas ejecutables. Un `match` exhaustivo protege aquí una frontera deliberadamente cerrada. |
| LSP | Una implementación sustituible debe respetar resultados, errores, dominio, efectos y tolerancias declaradas. Implementar el mismo `trait` no demuestra ese cumplimiento. |
| ISP | Exponer al evaluador una vista validada e inmutable; reservar la capacidad de construir a los consumidores que la necesitan. No exigir interfaces grandes ni un `trait` por función. |
| DIP | La composición neuronal depende del lenguaje y de la síntesis, no de detalles de cálculo `f64`. Una abstracción se introduce en una frontera de variación real, con contrato definido desde su consumidor. |

No se introduce un `trait` de operación ni un backend intercambiable en este cambio. Si más adelante aparece una variación real, se elegirá entre una función/cierre para comportamiento pequeño o un `trait` para un conjunto coherente de operaciones; el contrato y sus pruebas precederán a la elección de despacho estático o dinámico.

**Alternativas descartadas:** modelar cada instrucción como un objeto extensible, fragmentar funciones solo para bajar su métrica o extraer un `trait` por módulo. Ninguna de esas transformaciones establece por sí sola un contrato más útil.

### 4. Aislamiento de efectos y errores por etapa

Separar errores de construcción, compilación y evaluación mediante tipos o variantes distinguibles sin analizar mensajes. Cada fallo de un nodo ejecutado incluye su identificador del programa. Un fallo previo a disponer de nodo, como una lista de raíces vacía o un presupuesto rechazado, no inventa uno. El resultado de evaluación contiene todas las salidas en orden o un error; los buffers parciales son internos a la llamada.

La arena mantiene su presupuesto de construcción explícito; la compilación recibe su límite de nodos alcanzables y la evaluación su límite de nodos a ejecutar. Los nombres concretos de configuración se adaptan a la base. Los presupuestos de nodos no se presentan como límites exactos de bytes o de tiempo. Las posiciones de raíz también tienen coste, especialmente cuando contienen duplicados.

El evaluador usa almacenamiento temporal propio de cada llamada. No utiliza valores ni configuración mutable de otra ejecución. Las comprobaciones de positividad, no finitud y desbordamiento se conservan en los puntos definidos por la semántica del evaluador; un tipo estructural no las reemplaza. No se añaden clamps, constantes numéricas ocultas ni atajos mediante la referencia convencional.

La documentación actual de `f64::exp` y `f64::ln` declara precisión no especificada que puede variar incluso entre llamadas de una misma ejecución. Por tanto, aislamiento del estado y composición funcional no implican reproducibilidad numérica bit a bit. Las pruebas repetidas usan las tolerancias de las fixtures; no afirman transparencia referencial numérica estricta ni una tolerancia global para cualquier DAG.

Un fallo de síntesis puede dejar nodos internos válidos ya construidos en la arena, pero no entregar la raíz de la expansión fallida como éxito. Este cambio no exige un rollback transaccional de esos nodos; los programas validados anteriormente siguen intactos.

**Alternativa descartada:** convertir el programa validado en un certificado numérico o guardar una caché global de resultados. Ambas opciones mezclarían obligaciones estructurales y de ejecución y alterarían el contrato numérico sin justificación.

### 5. Estadisticas estructurales definidas por alcance

Proponer `ProgramStats` como resultado inmutable de inspección. Calcularlo al compilar el snapshot, sin exp/ln, y consultarlo después sin recorrer de nuevo el grafo:

- `V`: número de nodos únicos alcanzables.
- `E`: número de posiciones de operando de los nodos Magma alcanzables; dos operandos iguales cuentan dos enlaces.
- `R`: número de posiciones de raíz, incluidos duplicados.
- `D`: profundidad máxima en enlaces; One tiene profundidad 0 y Magma tiene 1 más el máximo de las profundidades de sus hijos.

El recorrido topológico evita recursión dependiente de la profundidad. En el modelo de coste por operación de tamaño fijo, el procesamiento estructural cuesta O(V + E + R); conservar nodos, valores temporales y raíces requiere almacenamiento lineal en V + R, dado que cada Magma tiene dos operandos. Son modelos de coste, no mediciones de tiempo o memoria real. El coste de producir la arena y sus nodos no alcanzables es adicional.

La demostración añade estas estadísticas por programa a sus presupuestos y errores numéricos existentes. La métrica ciclomática pertenece a una revisión del código; no se vuelve a atribuir al motor cada vez que cambia la entrada de la red.

**Alternativa descartada:** contar toda la arena, duplicar subárboles compartidos o medir profundidad con recursión. Esas opciones confundirían el programa seleccionado con su historial de construcción o introducirían límites de pila innecesarios.

### 6. Medicion ciclomatica como procedimiento de desarrollo

Usar `rust-code-analysis-cli` externamente, conservando su salida JSON. La documentación oficial confirma el formato del comando:

```sh
rust-code-analysis-cli -m -p src -O json -o target/analysis/cc
```

`docs/complexity.md` documentará la instalación reproducible, creación del directorio de salida y comandos de metadatos. El primer análisis se realizará después de que exista el núcleo, eligiendo y fijando una versión publicada o revisión Git exacta que reconozca su sintaxis. Se guardarán revisión de las fuentes, modificaciones locales, archivos incluidos, comando y procedencia del analizador junto a los resultados. Para fuentes modificadas, se conservará el diff y una copia de las fuentes adicionales necesarias para reconstruir el alcance, identificadas por su hash.

No basta con citar el número de versión desde una rama de desarrollo: la versión publicada 0.0.25 y el manifiesto actual de la rama consultada declaran ese mismo número pese a diferencias de edición y dependencias. La elección definitiva de la revisión se comprobará contra las fuentes reales; una revisión Git requiere registrar su commit exacto.

El alcance inicial es `src`; las pruebas inline que contenga se identifican en el reporte. `tests` y `examples` solo se analizarán como alcances adicionales explícitos. El análisis es de fuentes, no de código expandido por el compilador; las limitaciones sobre macros, cierres y agrupación de unidades se documentan según la revisión utilizada. Un error de parsing o un archivo omitido vuelve el reporte incompleto, no equivalente a complejidad cero.

La guía explicará cómo leer valores por función, método o cierre y los agregados que realmente exporte la herramienta, sin sumar contenedores con sus hijos. Se registran número de unidades y máximo observado; los promedios se interpretan con ese denominador. La fórmula ilustrativa de decisiones `sum(M_i - 1)` solo se utiliza bajo una convención local compatible y unidades no superpuestas, nunca como complejidad universal del sistema. La métrica tampoco determina un número suficiente de pruebas funcionales.

El procedimiento es documental y usa la herramienta existente; no requiere implementar un parser propio, integrarlo al motor ni añadir una barrera de CI por umbrales. Un reporte generado puede guardarse bajo `target/analysis/` y enlazarse como evidencia de desarrollo. La guía registra el resultado de la verificación de cobertura de archivos y sintaxis.

**Alternativas descartadas:** contar literalmente `if`, adoptar un umbral arbitrario o comparar revisiones con analizadores diferentes sin indicarlo. La disminución de una métrica solo inicia una revisión de contratos, cohesión y dependencias; no la sustituye.

### 7. Verificacion centrada en contratos

Combinar pruebas de integración como consumidor externo, pruebas internas para estructuras malformadas no construibles mediante la API pública y doctests de compilación correcta/`compile_fail`. Cada ejemplo negativo mantiene imports y preparación válidos y tiene un control positivo cercano que demuestra que el fallo se debe a la frontera examinada. En Rust estable, `compile_fail` por sí solo no verifica un código de error específico; se revisa el diagnóstico y no se atribuye esa garantía al atributo.

Casos clave:

- Handles de arenas distintas con posiciones locales iguales; raíces ajenas, lista vacía y raíces repetidas.
- Snapshot evaluable tras ampliar y liberar la arena; imposibilidad de fabricar o mutar un programa desde una crate consumidora.
- Ciclo y referencia inválida mediante fixtures internas, sin añadir puertas públicas de construcción inválida.
- Presupuestos pequeños y evaluaciones intercaladas que no contaminan programas ni llamadas posteriores.
- Inspección de programas que fallan numéricamente. Para evitar depender de la cancelación al construir cero, usar una expresión de argumento estrictamente negativo: sean a=M(1,1), b=M(a,1), c=M(b,1) y d=M(1,c); en reales d=e-exp(e)<0. M(1,d) debe fallar por dominio. La cadena M(c,1) fuerza desbordamiento en el evaluador f64. Estos casos se construyen con One/Magma, no con hojas numéricas.
- Estadísticas exactas para One, subexpresiones compartidas, raíces duplicadas y nodos no alcanzables. Una cadena profunda dentro del presupuesto ejercita el recorrido iterativo sin exigir evaluarla numéricamente.
- Regresión de las identidades y tres fixtures neuronales ya exigidas por la base, con sus tolerancias originales.

Se conserva la independencia de la referencia aritmética. No se agregan pruebas cuyo único propósito sea fijar un conteo ciclomático de la implementación.

**Alternativa descartada:** verificar solo resultados numéricos felices o confiar exclusivamente en doctests negativos. Ninguna de esas opciones demuestra procedencia, preservación de invariantes y aislamiento de etapas.

## Risks / Trade-offs

- [La base todavía no existe] -> Aplicar primero `implement-one-magma-eml`; si sus expansiones o tolerancias fallan, resolver esa dependencia antes de aplicar este cambio.
- [Duplicación de trabajo entre cambios] -> Reutilizar módulos, algoritmos y fixtures de la base; completar únicamente los contratos nuevos y consolidar sus consumidores en una misma migración de API.
- [Coste de snapshots] -> Explicitar la copia lineal y el alcance de la compartición; no prometer reducción de memoria ni introducir una caché antes de medir.
- [Falsa garantía por nombre de tipo] -> Reservar `CompiledProgram` para propiedades estructurales; mantener los fallos numéricos y la documentación de precisión.
- [Doctest que falla por un motivo incorrecto] -> Control positivo y revisión del diagnóstico; no depender de códigos de error que Rust estable no comprueba en ese atributo.
- [Analizador desactualizado o agregados engañosos] -> Fijar revisión verificada, registrar cobertura y conservar JSON original; declarar explícitamente cualquier análisis incompleto.
- [Sobreabstracción por seguir SOLID mecánicamente] -> Revisar qué contrato y motivo de cambio justifica cada frontera; mantener el enum cerrado y funciones concretas mientras satisfagan esa necesidad.

## Migration Plan

1. Confirmar que `implement-one-magma-eml` está implementado y que sus verificaciones pasan; registrar la revisión de partida. Si falta, detener la aplicación de este cambio con esa dependencia explícita.
2. Consolidar la frontera arena/programa, procedencia, snapshot y errores sobre los módulos existentes, migrando pruebas y ejemplo a la API validada.
3. Añadir inspección estadística y ampliar el reporte de la demostración sin alterar sus fórmulas ni tolerancias.
4. Publicar `docs/architecture.md` y `docs/complexity.md`, enlazarlas desde README y producir el primer reporte del núcleo real con su revisión del analizador fijada.
5. Ejecutar `cargo test --release`, `cargo test --doc`, `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings`, además del procedimiento de métricas documentado. Verificar tanto los escenarios nuevos como las fixtures previas.

No hay datos persistidos que migrar ni API actualmente publicada. Una reversión afecta conjuntamente los cambios de API y sus consumidores de este incremento, preservando la implementación base. Archivar los cambios en orden de dependencia cuando estén implementados y verificados; la disponibilidad de estos artefactos no demuestra cumplimiento de los contratos.

## References

- Diseño base: `../implement-one-magma-eml/design.md` y sus especificaciones completas.
- Precisión de [f64::exp](https://doc.rust-lang.org/std/primitive.f64.html#method.exp) y [f64::ln](https://doc.rust-lang.org/std/primitive.f64.html#method.ln).
- [Documentación de doctests](https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html) y [funciones inestables de rustdoc](https://doc.rust-lang.org/rustdoc/unstable-features.html).
- [Comandos oficiales de métricas](https://mozilla.github.io/rust-code-analysis/commands/metrics.html), [versión publicada 0.0.25](https://crates.io/api/v1/crates/rust-code-analysis-cli/0.0.25) y [manifiesto de desarrollo consultado](https://github.com/mozilla/rust-code-analysis/blob/master/rust-code-analysis-cli/Cargo.toml).
