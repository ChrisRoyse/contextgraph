#!/usr/bin/env bash
# Concatenate the arxiv paper sections into a single PAPER.md in assembly order.
# Output: PAPER.md in this directory.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="${HERE}/sections"
OUT="${HERE}/PAPER.md"

FILES=(
  "00_frontmatter.md"
  "01_introduction.md"
  "02_related_work.md"
  "03_framework_dda_mc.md"
  "04_systems.md"
  "06_method_tct.md"
  "07_case_studies.md"
  "08_discussion.md"
  "09_limitations.md"
  "10_conclusion.md"
  "12_competing_interests.md"
  "13_ethics.md"
  "11_references.md"
  "14_reproducibility.md"
)

missing=()
for f in "${FILES[@]}"; do
  [[ -f "${SRC}/${f}" ]] || missing+=("$f")
done
if (( ${#missing[@]} > 0 )); then
  printf 'error: missing file(s): %s\n' "${missing[*]}" >&2
  exit 1
fi

: > "${OUT}"

for f in "${FILES[@]}"; do
  cat "${SRC}/${f}" >> "${OUT}"
  if [[ -n "$(tail -c 1 "${SRC}/${f}")" ]]; then
    printf '\n' >> "${OUT}"
  fi
  printf '\n---\n\n' >> "${OUT}"
done

python3 - "${OUT}" <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1])
text = p.read_text()
trailer = "\n---\n\n"
if text.endswith(trailer):
    p.write_text(text[:-len(trailer)] + "\n")
PY

lines=$(wc -l < "${OUT}")
bytes=$(wc -c < "${OUT}")
words=$(wc -w < "${OUT}")
printf 'wrote %s (%s lines, %s words, %s bytes)\n' "${OUT}" "${lines}" "${words}" "${bytes}"
