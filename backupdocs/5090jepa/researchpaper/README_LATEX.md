# LaTeX Build Instructions

This directory contains the publication-ready two-column LaTeX
version of the paper alongside the markdown source sections.

## Files

- `paper.tex` — main LaTeX source (single self-contained file).
- `references.bib` — BibTeX bibliography (every URL was verified
  live during paper preparation).
- `Makefile` — build / clean targets.

## Build

```bash
cd docs/5090jepa/researchpaper
make           # produces paper.pdf
make view      # opens paper.pdf (Linux desktop)
make clean     # removes intermediate files
make veryclean # also removes paper.pdf
```

## Required system packages

A standard TeX Live install is enough. Specifically:

- `pdflatex` (TeX Live `texlive-latex-base`)
- `bibtex`
- LaTeX packages: `geometry`, `lmodern`, `microtype`, `graphicx`,
  `booktabs`, `tabularx`, `xcolor`, `tcolorbox`, `caption`,
  `amsmath`, `enumitem`, `url`, `authblk`, `titlesec`, `abstract`,
  `hyperref` (typically installed via
  `texlive-latex-extra` and `texlive-fonts-recommended`).

On Debian/Ubuntu:

```bash
sudo apt install texlive-latex-base texlive-latex-extra \
                 texlive-fonts-recommended
```

## Layout

The paper uses a single-column banner (title + authors + abstract)
spanning the full page width, then switches to a standard two-column
body. The idiom is:

```latex
\twocolumn[
  \maketitle
  \begin{onecolabstract} ... \end{onecolabstract}
]
\saythanks
```

(via the standard `abstract` package's `onecolabstract`
environment, which is the cleanest LaTeX idiom for a one-column
abstract in a two-column document).

The appendix re-enters single-column mode via `\onecolumn` so the
verbatim TOML and the long hash list don't break across narrow
columns.

## Section structure

| § | Title |
| - | --- |
| 1 | Introduction |
| 2 | Related Work |
| 3 | The Dynamic / ME-JEPA Runtime |
| 4 | The `career_taxonomy.v1` Public-Taxonomy Benchmark |
| 5 | Experiments |
| 6 | Full-State Verification |
| 7 | Bundles as a Training-Data Class |
| 8 | Results |
| 9 | Reproducibility Artifact |
| 10 | Discussion |
| 11 | Limitations |
| | Acknowledgments |
| | References |
| | Appendix A: domain pack TOML |
| | Appendix B: hash reference |
| | Appendix C: glossary |

## Markdown source

The original section-by-section markdown source files
(`00_main.md` through `13_appendix.md`) remain in this directory
for editing convenience. The LaTeX file is the publication-ready
version and is the canonical artifact for review.
