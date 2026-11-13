# Spec Delta

## Purpose

Ejecutar programas EML validados con un espacio numérico reutilizable, conservando resultados, errores e inspección y haciendo observable el coste adicional de planificación.

## ADDED Requirements

### Requirement: Plan derivado de un programa validado
La planificación SHALL aceptar únicamente un programa estructuralmente validado, conservar su orden de evaluación y completar sin evaluar valores numéricos. SHALL NOT permitir fabricar planes ejecutables desde instrucciones arbitrarias mediante la API pública segura.

#### Scenario: Programa con desbordamiento futuro
- **WHEN** se solicita planificar un programa válido cuya evaluación desborda, con límites suficientes
- **THEN** se obtiene el plan y el desbordamiento se detecta únicamente al ejecutarlo

#### Scenario: Fabricacion externa de instrucciones
- **WHEN** un consumidor intenta construir un plan escribiendo sus instrucciones privadas
- **THEN** el intento no compila

### Requirement: Semantica y orden de resultados preservados
La ejecución planificada SHALL evaluar cada nodo alcanzable una vez con la semántica One/Magma en `f64`, conservando orden y duplicados de raíces. SHALL conservar las tolerancias existentes y satisfacer una diferencia máxima de `1e-12 + 1e-12*abs(referencia)` frente al evaluador original en las fixtures finitas de equivalencia.

#### Scenario: Raices repetidas y One
- **WHEN** se ejecutan ambos evaluadores para las raíces [Magma(One,One), One, Magma(One,One)]
- **THEN** devuelven tres resultados en ese orden, conservan la repetición y satisfacen la tolerancia diferencial y el error original de 1e-12 respecto de e

### Requirement: Conservacion de valores vivos
El espacio numérico SHALL conservar un valor mientras tenga consumidores pendientes o sea una raíz de salida. La reutilización SHALL manejar operandos idénticos y lecturas de ambos operandos antes de sobrescribir su almacenamiento.

#### Scenario: Operandos repetidos en el ultimo consumo
- **WHEN** el último uso de t=Magma(One,One) ocurre en Magma(t,t)
- **THEN** ambos operandos reciben el valor de t y el resultado coincide con la evaluación de referencia

#### Scenario: Raiz intermedia aun necesaria
- **WHEN** se solicitan las raíces [t, Magma(t,t), t]
- **THEN** la ejecución conserva t hasta entregar las tres salidas, aunque su último uso como operando ya haya ocurrido

#### Scenario: Dependencias compartidas entre ramas
- **WHEN** dos ramas consumen una subexpresión común y sus resultados se combinan posteriormente
- **THEN** cada rama recibe el valor correcto y ningún almacenamiento se reutiliza antes de su último consumo

### Requirement: Reduccion comprobable del buffer numerico
El plan SHALL reutilizar almacenamiento cuando los valores dejen de ser necesarios. Para la cadena de aceptación t0=One, ti=Magma(One,ti-1), con 10000 pasos y solo la última raíz, SHALL requerir como máximo dos posiciones en su buffer de valores, frente a 10001 del evaluador original.

#### Scenario: Cadena larga con resultados transitorios
- **WHEN** se planifica y ejecuta la cadena de aceptación con límites suficientes
- **THEN** se informan como máximo dos posiciones numéricas y la salida satisface la tolerancia diferencial frente al evaluador original

### Requirement: Errores logicos y presupuestos independientes del ahorro
La ejecución SHALL mantener la categoría y el primer nodo lógico fallido del evaluador original. El presupuesto de evaluación SHALL contar nodos lógicos, incluido One, aunque haya menos posiciones numéricas. Los límites de planificación SHALL distinguir nodos y posiciones; un rechazo previo SHALL NOT inventar un nodo fallido.

#### Scenario: Presupuesto logico insuficiente
- **WHEN** el programa tiene más nodos que el límite de evaluación, aunque el buffer reutilizable sí quepa
- **THEN** se rechaza antes de ejecutar con el error de presupuesto lógico y sin identificador de nodo

#### Scenario: Fallo despues de reutilizar posiciones
- **WHEN** una dependencia produce un error de dominio o desbordamiento después de reutilizar almacenamiento
- **THEN** ambos evaluadores señalan la misma categoría y nodo resoluble por inspección, sin entregar salidas parciales exitosas

#### Scenario: Limite de posiciones insuficiente
- **WHEN** se intenta crear el plan con menos posiciones que las necesarias
- **THEN** se devuelve un error de planificación identificable y no un plan parcial ejecutable

### Requirement: Aislamiento de ejecuciones y snapshots
Cada ejecución SHALL poseer su estado numérico. Planificar, ejecutar, fallar o reintentar SHALL NOT modificar nodos, pares, raíces ni estadísticas del programa, ni conservar resultados numéricos para otras ejecuciones.

#### Scenario: Fallo intercalado y reintento
- **WHEN** se ejecuta un plan válido, otro falla y se vuelve a ejecutar el primero
- **THEN** sus resultados siguen cumpliendo la tolerancia diferencial y ambas inspecciones permanecen intactas

### Requirement: Contabilidad separada de almacenamiento
El reporte SHALL distinguir nodos, posiciones numéricas, bytes de valores, instrucciones del plan, raíces, metadatos y capacidad reservada. SHALL separar estos payloads del pico de memoria del proceso y SHALL NOT inferir aceleración o menor memoria total únicamente a partir de menos posiciones numéricas.

#### Scenario: Plan con metadatos adicionales
- **WHEN** se compara el plan con el evaluador original
- **THEN** se publican los bytes realmente ocupados por sus estructuras y el buffer numérico, incluyendo el programa residente, aunque el total del plan sea mayor
