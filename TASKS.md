# Prism Office — Task Plan

Unified 3-in-1 cross-platform office suite (Document / Workbook / Presentation)
on **Tauri v2 + Rust** with a **TypeScript + WebGPU/WebGL** presentation layer.

Status legend: `[x]` done · `[~]` in progress · `[ ]` todo
Task IDs are stable — reference them in commits/PRs (e.g. `P1-03`).

---

## Phase 0 — Repository & Tooling Foundation

- [x] **P0-01** Monorepo layout: Cargo workspace (`crates/*`, `src-tauri`) + pnpm frontend (`src/`).
- [x] **P0-02** TypeScript strict config, Vite build, Vitest unit tests.
- [x] **P0-03** Rust lint/format baseline (`rustfmt`, `clippy -D warnings`).
- [x] **P0-04** CI workflow: Rust tests + clippy, frontend typecheck + tests + build.
- [x] **P0-05** Pre-commit hooks (fmt, typecheck) and conventional-commit check (`.githooks/`, enabled by `pnpm install`).
- [x] **P0-06** Architecture Decision Records folder (`docs/adr/`) — ADR-0001 stack choice, ADR-0002 lossless OpenXML model.

## Phase 1 — Shell & Hub Runtime

### 1A. Native host (Tauri v2 / Rust)
- [x] **P1-01** Tauri v2 host crate (`src-tauri`) with main window config & capabilities.
- [x] **P1-02** IPC bridge: typed commands `open_document`, `save_document`, `new_document`, `app_info`.
- [x] **P1-03** File I/O layer: read/write packages with atomic save (temp file + rename).
- [x] **P1-04** Native window styling: macOS vibrancy/traffic-light inset, Windows Mica/Snap layouts.
- [x] **P1-05** Contextual multi-instance windows (one native window per document, `WebviewWindowBuilder`).
- [x] **P1-06** Shared background daemon registry (AI, Python sandbox, recovery) with event bus.
- [x] **P1-07** Native open/save dialogs + OS file association registration (.docx/.xlsx/.pptx).
- [x] **P1-08** Recent files store & single-instance handling (open file from OS into existing app).

### 1B. OpenXML packaging engine (`crates/prism-openxml`)
- [x] **P1-10** OPC package reader: ZIP container, `[Content_Types].xml`, part map.
- [x] **P1-11** Relationships (`_rels/*.rels`) parser/serializer, part-relative target resolution.
- [x] **P1-12** Document kind detection (docx/xlsx/pptx) via main-part content type.
- [x] **P1-13** OPC package writer with deterministic ordering & round-trip test.
- [x] **P1-14** Minimal blank document templates (docx/xlsx/pptx) generated in Rust.
- [x] **P1-15** Package validation (missing content types, dangling rels, zip-bomb/size limits, path traversal).
- [x] **P1-16** Zero-copy part access (`bytes::Bytes` / mmap for large packages).
- [x] **P1-17** Streaming reader for very large `.xlsx` (sheet XML SAX pull parser).

### 1C. Application Hub & shell UI (TypeScript)
- [x] **P1-20** Application Hub gateway: mode cards (Document / Workbook / Presentation), open file, recent list.
- [x] **P1-21** Adaptive theming: Cobalt (Word), Emerald (Excel), Terracotta (PowerPoint) accent tokens.
- [x] **P1-22** Mode-specific ribbon definitions (Typography/Page Setup/Citations/Structure; Data/Formulas/Python/Pivot; Design/Cameo/Transitions/Recording).
- [x] **P1-23** Tabbed MDI: tab model (open/close/activate/dirty), ribbon & engine swap on tab switch.
- [x] **P1-24** Host bridge abstraction (Tauri IPC vs. browser fallback for dev/tests).
- [x] **P1-25** Windowing topology preference (Tabbed MDI ⇄ Multi-Instance) in settings.
- [x] **P1-26** Keyboard shortcuts & command palette.
- [x] **P1-27** i18n framework (en, az) and RTL readiness.

## Phase 2 — Word Processing Engine

- [x] **P2-01** Document model (blocks/inlines/runs, styles) with immutable transactions + undo/redo.
- [x] **P2-02** DOCX import: `word/document.xml`, `styles.xml`, numbering, sections, headers/footers, footnotes/endnotes, comments, settings, bibliography sources.
- [x] **P2-03** DOCX export with round-trip fidelity tests — synthetic fixtures, TS⇄Rust contract, and an 11-file real-world corpus (Apache POI, python-docx) checked for model stability, unchanged parts and idempotence. Broader Microsoft 365 / Google / iWork corpus is X-04.
- [x] **P2-04** Paragraph layout engine: UAX #14-subset breaking, soft + Liang auto-hyphenation, alignment/justification, tab stops with leaders, bidi reordering (UAX #9 L2 on fragments). See ADR-0003 for scope.
- [x] **P2-05** Fonts: cached canvas metrics, platform shaping (ADR-0003 — rustybuzz deliberately not bundled), metric-compatible fallback stacks, embedded DOCX fonts de-obfuscated and loaded via FontFace.
- [x] **P2-06** Pagination: page size/orientation, margins, columns (with separators), page/column breaks, keep-with-next, widow/orphan control, repeated table header rows, footnote area.
- [~] **P2-07** Renderer: page raster cache + WebGL2 compositor (default) with Canvas 2D fallback, overlay caret/selection, hit-testing, IME. WebGPU backend implemented but opt-in — untested on real GPUs (blank under headless SwiftShader).
- [x] **P2-08** Typography ribbon wired to commands (font, size, bold/italic, styles, lists).
- [x] **P2-09** Markdown ⇄ OpenXML bidirectional converter + split-pane Markdown source view.
- [x] **P2-10** Clipboard interop: HTML (incl. tables), RTF (read/write), plain text, native Prism fragments, pasted images.
- [x] **P2-11** Tables (edit, rows/columns, merge/split, shading, header rows), pictures (insert/scale/alt text), shapes, footnotes/endnotes, headers/footers with PAGE/NUMPAGES, TOC and citation/bibliography fields, source manager.
- [x] **P2-12** AI Agent panel: drafting from outline + reference files with streamed preview (Claude via the native AI daemon). Not exercised against the live API in this environment (no key).
- [x] **P2-13** Verifiable citations: `[[n|quote]]` markers become footnotes + bibliography sources; quotes are checked verbatim against the reference text and unverified ones are flagged.
- [x] **P2-14** Rewriting in six tones with a word-level inline diff to accept or discard.
- [x] **P2-15** Ink overlay (pointer/pen), ink-to-shape (line, arrow, triangle, rectangle, diamond, ellipse → DrawingML shapes), handwriting-to-text via the AI daemon's vision model.
- [x] **P2-16** Accessibility checker: contrast, heading order/empty headings, missing alt text (manual or AI-generated), table headers, vague links, small text, LIX/Flesch readability.
- [x] **P2-17** Track changes (record, show/hide markup, accept/reject one/all, next change) and comments (anchor, side panel, edit, delete) with `w:ins`/`w:del`/comments.xml round-trip.

## Phase 3 — Spreadsheet Calculation Engine

- [ ] **P3-01** Cell store: sparse columnar storage, shared strings, styles, number formats.
- [ ] **P3-02** XLSX import/export (sheets, shared strings, styles, defined names, tables).
- [ ] **P3-03** Formula lexer/parser (A1/R1C1, structured refs, array constants, LAMBDA/LET).
- [ ] **P3-04** Dependency graph + incremental recalculation, cycle detection, multithreaded calc.
- [ ] **P3-05** Dynamic array evaluation & spill engine (`#SPILL!`, `#` spill-range operator).
- [ ] **P3-06** Core function library (math, stat, text, logical, date/time, lookup).
- [ ] **P3-07** XLOOKUP / XMATCH incl. regex match mode (`match_mode = 3`).
- [ ] **P3-08** REGEXTEST / REGEXEXTRACT / REGEXREPLACE.
- [ ] **P3-09** GROUPBY / PIVOTBY / PERCENTOF / TRIMRANGE.
- [ ] **P3-10** Virtualized grid canvas (frozen panes, merged cells, 1M+ rows smooth scroll).
- [ ] **P3-11** Cell editing, formula bar, autocomplete, fill handle, selection model.
- [ ] **P3-12** Python sandbox daemon (isolated subprocess, pandas/numpy/scipy/matplotlib).
- [ ] **P3-13** `=PY(expression, return_type)` — DataFrame spill & plot image output.
- [ ] **P3-14** Charts (DrawingML chartML) and pivot tables UI.
- [ ] **P3-15** Data connections (CSV, JSON, SQL via ODBC) & refresh.

## Phase 4 — Presentation & Vector Engine

- [ ] **P4-01** Shared DrawingML engine crate (shapes, geometry presets, fills, gradients, effects, 3D).
- [ ] **P4-02** PPTX import/export (slides, layouts, masters, themes, notes).
- [ ] **P4-03** GPU slide compositor (WebGPU) with thumbnails rail and zoom.
- [ ] **P4-04** Slide editing: selection, transform handles, snapping, alignment guides.
- [ ] **P4-05** Animation timeline & transitions, incl. Morph.
- [ ] **P4-06** Slideshow mode + presenter view (dual display).
- [ ] **P4-07** Cameo live camera object (vector-masked frames, effects, morphs across slides).
- [ ] **P4-08** Native capture: AVFoundation (macOS) / Media Foundation (Windows).
- [ ] **P4-09** Recording Studio: multi-track camera/mic/ink/transition timeline.
- [ ] **P4-10** Teleprompter in presenter view (positioned under webcam).
- [ ] **P4-11** MP4 export 1080p/4K with chapter bookmarks.
- [ ] **P4-12** AI presentation generator (from text/bullets/linked docx).
- [ ] **P4-13** Speaker Coach (pace, pitch, filler words, engagement).
- [ ] **P4-14** Live Presenting: QR audience link + live translated subtitles.

## Cross-cutting

- [ ] **X-01** Local-first auto-recovery: differential transaction journal every 3 s, crash restore.
- [~] **X-02** AI service daemon: Claude Messages API with streaming, refusal fallbacks, key kept in the native host (env or 0600 file). Pending: OS keychain storage, rate-limit/backoff handling, provider abstraction.
- [ ] **X-03** Telemetry-free crash reporting (opt-in), structured logging.
- [ ] **X-04** OpenXML conformance corpus tests (Microsoft 365, Google Workspace, iWork exports).

## Phase 5 — Cross-Platform Hardening & Release

- [ ] **P5-01** Apple Silicon & Windows ARM64 builds and performance profiling.
- [ ] **P5-02** Installers: `.dmg` (macOS), `.msi` (Windows) via Tauri bundler.
- [ ] **P5-03** Code signing & Apple notarization; Windows Authenticode.
- [ ] **P5-04** Auto-update channel (Tauri updater, signed manifests).
- [ ] **P5-05** Enterprise security audit (sandbox escape, file parser fuzzing, CSP).
- [ ] **P5-06** Fuzzing harness for OpenXML parsers (`cargo-fuzz`).

---

## Progress log

- **2026-11-12** — Phase 2 complete except WebGPU validation (P2-07): tables,
  pictures, shapes, notes, headers/footers, fields, tracked changes, comments,
  columns, hyphenation, bidi, RTF, GPU compositor, AI drafting/rewriting with
  verified citations, ink, accessibility checker, real-world DOCX corpus.

- **2026-11-12** — Phase 0 complete (git hooks, ADRs). Phase 2 started:
  `prism-word` crate (lossless DOCX ⇄ model), canvas WYSIWYG editor with
  undo/redo, layout & pagination, typography/page-setup ribbon, Markdown
  split view, HTML clipboard.

- **2026-11-12** — Phase 1 complete: window styling (macOS overlay/traffic
  lights, Windows Mica), per-document native windows + tab detach, shared
  daemon registry with event bus (AI/Python/recovery registered as planned
  stubs), native dialogs, OS file-open handling (argv, single-instance,
  macOS `Opened`), persistent recent files & settings, zero-copy/mmap part
  access, streaming worksheet reader, windowing preference, command palette &
  shortcuts, en/az i18n with RTL-ready CSS. Rust cross-checked for
  `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`; CI runs on all three OSes.
- **2026-11-12** — Task plan created. Phase 0 + core of Phase 1 started:
  workspace scaffold, `prism-openxml` (read/write/validate/detect/templates),
  Tauri host with IPC commands, Hub + adaptive theming + ribbons + tabbed MDI.
