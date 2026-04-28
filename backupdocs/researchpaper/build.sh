#!/usr/bin/env bash
# Concatenate the research paper files into a single Markdown document in order.
# Skips README (index) and _evidence_*.md (research pools, not part of the manuscript).
# Output: PAPER.md in this directory.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${HERE}/PAPER.md"

FILES=(
  "00_frontmatter.md"
  "01_introduction.md"
  "02_related_work.md"
  "03_framework_dda_mc.md"
  "04_system_contextgraph.md"
  "05_system_clipcannon.md"
  "06_method_tct.md"
  "07_case_shakespeare.md"
  "08_case_santa_lora.md"
  "09_case_voice_clone.md"
  "10_discussion.md"
  "11_leapable.md"
  "12_agi_and_teleox.md"
  "13_references.md"
  "14_appendix_schemas.md"
  "15_appendix_repro.md"
)

# Verify every file exists before writing anything.
missing=()
for f in "${FILES[@]}"; do
  [[ -f "${HERE}/${f}" ]] || missing+=("$f")
done
if (( ${#missing[@]} > 0 )); then
  printf 'error: missing file(s): %s\n' "${missing[*]}" >&2
  exit 1
fi

: > "${OUT}"

for f in "${FILES[@]}"; do
  cat "${HERE}/${f}" >> "${OUT}"
  # Blank line between sections, only if the file does not already end with one.
  if [[ -n "$(tail -c 1 "${HERE}/${f}")" ]]; then
    printf '\n' >> "${OUT}"
  fi
  printf '\n---\n\n' >> "${OUT}"
done

# Strip the trailing separator so the document ends cleanly.
# (Removes the last "---\n\n" block written above.)
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
printf 'wrote %s (%s lines, %s bytes)\n' "${OUT}" "${lines}" "${bytes}"
