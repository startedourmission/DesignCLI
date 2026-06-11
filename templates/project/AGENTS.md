# DesignCLI Project Terminal Guide

This file is copied into each `.dxdoc` project folder so embedded Codex or
Claude Code sessions can immediately see the live-editing rules from their
working directory.

## Live Editing

The embedded terminal starts inside the active `<name>.dxdoc` folder. The daemon
sets these variables:

- `DX_DOC_ID`: the live document id.
- `DX_DOC`: the repo-relative document path, for example `projects/demo.dxdoc`.
- `DX_PROJECT_DIR`: the absolute project folder.
- `DX_CLI_GUIDE`: this guide file.

- `DX_SERVER`: the live daemon URL (preset).
- `dx` is on `PATH` (next to the daemon binary).

**Every `dx` command in this terminal is live by default** — `--server`/`--doc`
fall back to `DX_SERVER`/`DX_DOC` env vars, so plain commands reflect instantly
in the open editor:

```bash
dx layer list
dx draw text 120 220 "제목" --size 64 --color 24,28,32,255
dx layer style 3 --shadow "0,8,24,10,14,20,110"
```

Expected write output is `(라이브 적용)`. Treat `(디스크 저장)` as a problem for
live work because the web UI did not receive the edit (env was overridden).

## Useful Commands

```bash
dx doc info
dx layer list
dx layer style <id> --fill 255,255,255,255 --radius 24
dx layer text <id> --text "새 내용" --bg 255,213,95,255
dx export png /tmp/designcli-export.png
```

For engine or web code changes, work from the repository root and run:

```bash
bash scripts/check.sh
```
