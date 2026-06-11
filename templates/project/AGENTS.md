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

For browser-visible edits, run the CLI through the daemon:

```bash
cd ../..
target/debug/dx --server http://localhost:8137 --doc "$DX_DOC" <command>
```

Expected write output is `(라이브 적용)`. Treat `(디스크 저장)` as a problem for
live work because the web UI did not receive the edit.

## Useful Commands

```bash
cd ../..
target/debug/dx --server http://localhost:8137 --doc "$DX_DOC" layer list
target/debug/dx --server http://localhost:8137 --doc "$DX_DOC" doc info
target/debug/dx --server http://localhost:8137 --doc "$DX_DOC" export png /tmp/designcli-export.png
```

For engine or web code changes, work from the repository root and run:

```bash
bash scripts/check.sh
```
