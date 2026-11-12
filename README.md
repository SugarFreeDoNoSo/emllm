# EMLLM

Redes neuronales expresadas únicamente con **One** y **Magma**, donde
`Magma(x, y) = exp(x) - ln(y)` y `y > 0`.

El proyecto está en fase de diseño: la primera implementación propuesta es un
núcleo Rust y una red 2-2-1 cuyos pesos, entradas y operaciones se expanden a
One/Magma. La planificación está en
[`openspec/changes/implement-one-magma-eml/`](openspec/changes/implement-one-magma-eml/).

## Informe técnico

- [Fuente LaTeX con diagramas TikZ](docs/report/emllm.tex).
- [Descargar el PDF en Releases](https://github.com/SugarFreeDoNoSo/emllm/releases).
- [Compilaciones de GitHub Actions](https://github.com/SugarFreeDoNoSo/emllm/actions/workflows/report.yml).

Para compilar localmente, instala TeX Live con español, TikZ, Latin Modern y
`latexmk`, y ejecuta:

```sh
cd docs/report
mkdir -p build
latexmk -pdf -interaction=nonstopmode -halt-on-error -file-line-error -outdir=build emllm.tex
```

El PDF queda en `docs/report/build/emllm.pdf` (desde la raíz del repositorio).
GitHub Actions compila cambios del informe en `main`, pull requests y ejecuciones
manuales, y guarda el PDF como artefacto. Para publicar una versión documental:

```sh
git tag -a report-v0.1.0 -m "Informe conceptual EMLLM 0.1.0"
git push origin report-v0.1.0
```

Usa un tag nuevo para cada versión y actualiza la versión documental del `.tex`.
Los tags `report-v*` crean una release con el PDF y su checksum SHA-256 tras una
compilación satisfactoria. Una release existente se conserva al repetir el job.
