# Prism Office

Unified 3-in-1 cross-platform office suite — **Document** (Word), **Workbook**
(Excel) and **Presentation** (PowerPoint) — in one desktop app for macOS and
Windows, built on **Tauri v2 + Rust** with a **TypeScript** canvas shell.

See [`TASKS.md`](./TASKS.md) for the full phased task plan and progress.

## Layout

```
crates/prism-openxml/   OPC/OpenXML package engine (read, write, validate, templates)
src-tauri/              Native host: window, IPC commands, document registry
src/core/               Modes, ribbons, tab model, host bridge, theming
src/hub/                Application Hub (mode selection gateway)
src/shell/              Tabbed-MDI shell + ribbon
src/modes/              Engine views (placeholders until Phases 2–4)
```

## Develop

Prerequisites: Rust (stable), Node 22, pnpm 10. On Linux also the WebKitGTK
dev packages (`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`).

```bash
pnpm install
```

```bash
pnpm tauri dev      # native app
```

```bash
pnpm dev            # browser preview (in-memory host, no file I/O)
```

Tests:

```bash
cargo test --workspace && pnpm test
```
