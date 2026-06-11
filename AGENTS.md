# Agent Instructions

## Live Design Work

When creating or editing a design that the user expects to watch in the web UI, use the live daemon path from the first command.

- Start or verify `dx-daemon` before making edits.
- Use `target/debug/dx --server http://localhost:8137 --doc projects/<name>.dxdoc ...` for every write command, including `doc create`.
- Do not rely on automatic server detection for new projects. New `.dxdoc` paths can fall back to disk mode before the web app has opened them.
- Treat CLI output that says `(디스크 저장)` as a problem for live work. The expected write output is `(라이브 적용)`.
- Use the same project id in the browser and CLI. For `projects/foo.dxdoc`, the live document id is `foo`.
- If a document was accidentally edited in disk mode while the web has it open, inspect server state with `dx --server http://localhost:8137 --doc projects/<name>.dxdoc layer list` before continuing. Reapply missing edits through `--server` so the web receives broadcasts.

For card news or social image tasks, still export and verify the final PNG, but keep the web-visible live document as the primary editing surface during the process.

## Engineering Loop (code changes)

For engine/web code work (not design work), close every change cycle with the gate:

1. `bash scripts/check.sh` — workspace tests, wasm build, JS syntax, coordinate-invariant
   regression (`verify_fixes`), render bench, and visual artifacts.
2. Inspect `/tmp/dcli-scene-{fit,100,400}.png` by actually opening the images — judge
   aliasing, text sharpness, and layout drift at zoom-out / device-1:1 / 4x.
3. If `crates/*` changed: rebuild wasm (`bash dx-web/scripts/build-wasm.sh`), restart the
   daemon, and tell the user to hard-refresh. A stale daemon rejects new Actions and the
   web silently rolls edits back.
4. Track perf trends with `node dx-web/bench_composite.mjs` (do not let view composite
   times regress materially).

Contracts and architecture context live in `CLAUDE.md` (root).
