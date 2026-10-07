#!/usr/bin/env bash
# check_regress.sh — pixel gate for EVERY change to jplot.
#
# Method (established in this project): render all compare/specs/*.json with
# jplot, rasterise the svglite references with the SAME resvg, and diff.
# Two modes:
#   ./check_regress.sh            run comparison, print table + assert mean
#   ./check_regress.sh --snap     freeze the current table as the baseline
# Baseline lives in compare/baseline_diffs.txt (commit it deliberately).
set -euo pipefail
cd "$(dirname "$0")/.."

PY=/opt/anaconda3/bin/python3
R=/opt/anaconda3/envs/rref/bin/Rscript

if [[ ! -f compare/specs/01_scatter.json ]]; then
  echo "regenerating R refs…"
  (cd compare && "$R" export_specs.R)
fi

cargo build --release --quiet

for f in compare/specs/*.json; do
  n=$(basename "$f" .json)
  ./target/release/jplot render "$f" -o "compare/ours/$n.png" --scale 1 >/dev/null
done
for f in compare/refs/*.svg; do
  n=$(basename "$f" .svg)
  out="compare/refs_png/$n.png"
  [[ -f "$out" && "$out" -nt "$f" ]] || \
    ./target/release/jplot render-svg "$f" -o "$out" --scale 1 >/dev/null
done

(cd compare && "$PY" compare_images.py | grep -E '^[0-9]' | awk '{print $1, $2}') \
  > /tmp/diff_now.txt

if [[ "${1:-}" == "--snap" ]]; then
  cp /tmp/diff_now.txt compare/baseline_diffs.txt
  echo "baseline frozen: compare/baseline_diffs.txt"
  exit 0
fi

if [[ ! -f compare/baseline_diffs.txt ]]; then
  echo "no baseline; run ./scripts/check_regress.sh --snap first" >&2
  exit 2
fi

# behaviour-preserving refactors must be byte-identical; feature work relaxes
# to the mean gate below.
if diff -q /tmp/diff_now.txt compare/baseline_diffs.txt >/dev/null; then
  echo "ZERO-CHANGE: identical to baseline"
fi

awk '{gsub("%","",$2); s+=$2; n++; if($2>m){m=$2; fig=$1}} END {
  printf "mean %.3f%% over %d figures (max %s %.2f%%)\n", s/n, n, fig, m
  exit !(s/n < 1.0)
}' /tmp/diff_now.txt
