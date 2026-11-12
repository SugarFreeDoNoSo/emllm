# Proposal

## Why

El núcleo One/Magma está planificado, pero sus fronteras de tipos, responsabilidades y medición todavía no tienen contratos suficientemente concretos para guiar una implementación idiomática en Rust. Queremos convertir la exploración sobre SOLID, composición funcional y complejidad ciclomática en garantías comprobables, distinguiendo encapsular decisiones, eliminar comprobaciones redundantes y conservar las comprobaciones numéricas necesarias.

## What Changes

- Establecer una frontera pública entre construcción y programa estructuralmente validado, con referencias opacas, procedencia de arena comprobada y una representación ejecutable inmutable.
- Hacer que la evaluación reciba únicamente programas validados y configuración explícita; conservar los errores numéricos de dominio, desbordamiento y no finitud como resultados identificables.
- Delimitar las responsabilidades de expresiones, síntesis, compilación, evaluación y red, usando un lenguaje cerrado con `enum`, funciones para composición y métodos para proteger invariantes. Documentar cuándo un `trait` aportaría un contrato de sustitución real.
- Exponer estadísticas estructurales reproducibles de los nodos alcanzables y un procedimiento de análisis ciclomático que identifique herramienta, versión, alcance y limitaciones de comparación.
- Verificar los contratos mediante pruebas de API, ejemplos que deben compilar o fallar al compilar y regresiones de las fixtures del núcleo y la red.
- Documentar cómo aplicar los cinco principios SOLID junto con programación funcional, sin equiparar menor complejidad local con menor coste computacional ni con mayor corrección.

## Capabilities

### New Capabilities

- `eml-program-contracts`: ciclo de vida público de un programa validado, aislamiento entre arenas, preservación de invariantes y separación entre garantías estructurales y evaluación numérica.
- `eml-complexity-reporting`: estadísticas estructurales consultables sin evaluar y reportes reproducibles que distinguen complejidad del código, tamaño del programa y evidencia numérica.

### Modified Capabilities

Ninguna. El inventario principal de especificaciones está vacío; las capacidades `eml-expressions` y `eml-network-inference` pertenecen al cambio pendiente `implement-one-magma-eml`. Esta propuesta añade contratos complementarios a esa base.

## Impact

- **Dependencia de implementación:** completar primero `implement-one-magma-eml`, incluidos su núcleo Rust y sus criterios de aceptación. Esta propuesta no constituye una implementación alternativa del motor.
- **Áreas previstas:** API pública y módulos del futuro crate, pruebas de integración y documentación ejecutable, ejemplo neuronal, README y guías de arquitectura y complejidad.
- **Compatibilidad:** actualmente no hay Cargo, fuentes Rust ni API publicada. La consolidación de la API deberá migrar los consumidores y ejemplos introducidos por el cambio base antes de publicar el crate.
- **Herramientas:** biblioteca estándar en ejecución; `rust-code-analysis-cli` como herramienta externa de desarrollo con revisión identificada, sin dependencia de ejecución del motor.
- **Alcance:** arquitectura y observabilidad del núcleo inicial. Entrenamiento, nuevas instrucciones EML, plugins, evaluadores alternativos, aceleración y umbrales automáticos de calidad quedan fuera de este cambio.
