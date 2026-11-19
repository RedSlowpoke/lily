#!/usr/bin/env bash
# Builds the spike, the harness's engine, with the engine's build
# (scripts/build.mjs: chapter 08 §1.3 into packages/engine/dist/engine/, then
# the size report), then the studio's site, which bundles engine.worker.js and
# copies the engine under hashed names, each with its .br, into
# packages/studio/dist/site/. Writes engine.worker.js.br beside the worker, so
# that serve.mjs compresses nothing (H5), and prints the worker's raw, gzip and
# brotli bytes. serve.mjs serves the result to the phones: scheme.html,
# translate.html and restore.html load the core from dist/engine/ (core.wasm,
# core.image, scheme.image), and
# translate.html the corpus's scores (npm run corpus) and the installed
# LilyPond's ly/ and scm/ files they ask for. It also builds crates/svg's write
# example, whose pages of step 6's references (npm run capture) serve.mjs
# writes at start for svg.html to check the core's against (MS3 step 9).
#
#   WASM_OPT, FONT, VARIANT   as scripts/build.mjs says (VARIANT: ia, stable, simd)
#   SITE=0                    skips the site build
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
engine=$(cd "$here/../../.." && pwd)
studio=$(cd "$engine/../studio" && pwd)

node "$engine/scripts/build.mjs"
node -e "import(process.argv[1]).then((m) => m.example('write'))" "$engine/tools/examples.mjs"

if [ "${SITE:-1}" != 0 ]; then
  (cd "$studio" && node esbuild.mjs --site --production >/dev/null 2>&1)
  f=$studio/dist/site/engine.worker.js
  # Written, not made with -o, which copies the file's time: serve.mjs takes an older .br for a stale one.
  brotli -q 11 -c "$f" >"$f.br"
  echo
  printf "%-22s %8s %8s %8s\n" file raw gzip brotli
  printf "%-22s %8s %8s %8s\n" "$(basename "$f")" "$(wc -c <"$f" | tr -d ' ')" "$(gzip -9c "$f" | wc -c | tr -d ' ')" "$(wc -c <"$f.br" | tr -d ' ')"
fi
