# Design

## Context

El repositorio contiene README mínimo, licencia y configuración OpenSpec; no hay Cargo, fuentes, pruebas ni checkpoints. El ZIP mencionado no es accesible en esta sesión. Véase proposal.md para la motivación. Este diseño es necesario por el nuevo modelo de representación y la separación entre semántica real y evaluación finita.

## Goals / Non-Goals

**Goals:** establecer un núcleo inspeccionable, con composición simbólica previa a la evaluación, y una demostración pequeña de inferencia pura.

**Non-Goals:** representar exactamente cualquier real mediante una expresión finita; importar automáticamente el prototipo; demostrar ventajas de rendimiento. La completitud se toma como premisa, pero cada expansión utilizada necesita una derivación y una comprobación independiente.

## Decisions

### 1. DAG inmutable con solo dos formas

Usar una arena de nodos `One | Magma(Id, Id)` y raíces explícitas. Internar por estructura y orden de hijos, nunca por resultado numérico. Los constructores solo aceptan referencias existentes de la misma arena; validar raíces y pertenencia antes de compilar. El evaluador recorrerá el orden topológico iterativamente. Una interfaz de inspección permitirá comprobar la pureza.

No se adoptan Constant, Input, Parameter, Add, Mul ni Neg como instrucciones. Las funciones de construcción con esos propósitos serán macros que devuelven nodos One/Magma. Un árbol sin compartición sería más sencillo, pero duplicaría expansiones innecesariamente.

### 2. Valores de frontera racionales, programas cerrados

La API de construcción acepta racionales normalizados, inicialmente acotados al dominio mínimo de las especificaciones. Los enteros de esa API son metadatos de construcción, no hojas ejecutables. La síntesis será constructiva, mediante identidades algebraicas documentadas; no una búsqueda exhaustiva ni una tabla que inyecte resultados en el evaluador.

Las expansiones de suma, negación, producto y racionales deben tener derivaciones que expliquen las condiciones de positividad de cada logaritmo. Como anclas comprobables: sea M(x,y)=exp(x)-ln(y); e=M(1,1), exp(e)=M(e,1), y cero=M(1,M(e,1)). Estas igualdades son en aritmética real; cero puede sufrir cancelación en aritmética finita. No reemplazar silenciosamente su valor numérico por una constante cero.

Antes de componer la red, verificar las expansiones concretas en el dominio de aceptación. La completitud por sí sola no prescribe un algoritmo de síntesis eficiente. Si no se consigue satisfacer el contrato con las expansiones elegidas, registrar el fallo y revisar el diseño, sin introducir primitivas adicionales como atajo.

### 3. Evaluador numérico explícito

Primera implementación Rust con biblioteca estándar y `f64`. Solo el evaluador de Magma calcula exp y ln; las operaciones internas necesarias para evaluar esa semántica no son instrucciones adicionales del lenguaje. No se simplifica una macro a aritmética convencional durante ejecución. Conservar valores de todos los nodos inicialmente facilita diagnósticos; reutilizar temporales se deja para una optimización posterior.

Comprobar finitud de operandos y resultados, positividad del segundo operando y desbordamiento de exp. Informar nodo y categoría de error. El subdesbordamiento y la cancelación se documentarán como limitaciones: las tolerancias se verifican para las fixtures, no son una garantía global de exactitud. No aplicar clamps ni tolerancias para convertir argumentos logarítmicos inválidos en válidos.

Construcción y evaluación reciben presupuestos de nodos; el recorrido iterativo evita depender de la pila para profundidad. Las tolerancias de las especificaciones son criterios de aceptación, no resultados obtenidos. La precisión arbitraria se pospone; si f64 no satisface las fixtures, se requiere revisar explícitamente esta decisión antes de declarar completado el cambio.

### 4. Red pequeña como integración

Usar el caso fijo 2-2-1 definido en la especificación, con suma afín, activación exp(-z*z) y salida afín. Cada entrada produce raíces cerradas; los pesos también se sintetizan. No habrá parámetros mutables enlazados. El constructor puede compartir expresiones inmutables entre inferencias, pero nunca requiere ejecutar una capa para construir la siguiente.

```text
racionales --> constructores --> DAG One/Magma completo --> evaluador --> salida
     |
     +--> referencia aritmetica independiente -----------------------> comparacion
```

La referencia convencional existe solo en pruebas/demostración. La demostración muestra error absoluto, cantidad de nodos y presupuestos, sin afirmar que menos nodos o memoria equivalgan a mayor velocidad.

### 5. Organización propuesta

Crear Cargo.toml, src/lib.rs y módulos de expresiones, síntesis, evaluación y red; incluir un ejemplo ejecutable y pruebas de integración. Usar nombres concretos consistentes durante implementación. Actualizar README con semántica, comandos y limitaciones. No hay APIs anteriores que preservar ni dependencias externas previstas.

## Risks / Trade-offs

- [Expansiones con cancelación o grandes exponenciales] -> Derivar condiciones de dominio y comprobar primero escalares y luego la red; un fallo numérico no se oculta tras un resultado de referencia.
- [Explosión del tamaño] -> Compartición estructural y presupuestos explícitos; no prometer escalado a la red de 141.824 parámetros.
- [Confundir código estructural de bytes con valores EML] -> No incorporar el codec; cada hoja de esta red deriva de One con semántica numérica documentada.
- [Coste de nuevas entradas] -> Reconstruir raíces cerradas, documentando el coste en lugar de reintroducir variables ocultas.
- [Resultados externos no reproducidos] -> Ejecutar pruebas Rust propias; no reutilizar como evidencia los números del informe pegado.

## Migration Plan

Implementación aditiva en un repositorio sin aplicación previa. No hay datos que migrar. Validar el núcleo antes de añadir la demostración; una reversión elimina únicamente los archivos introducidos por la implementación y conserva estos artefactos de planificación.
