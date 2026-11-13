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

- [x] **P3-01** Cell store: sparse columnar storage, shared strings, styles, number formats. _(sparse ordered map + per-column formula index; filled-down formulas share one parsed tree; Excel number formats; font/fill/border/alignment styles)_
- [x] **P3-02** XLSX import/export (sheets, shared strings, styles, defined names, tables). _(lossless passthrough of unowned parts/children; shared & dynamic-array formulas, `_xlfn/_xlws/_xlpm/_xleta`, metadata.xml; verified with openpyxl + LibreOffice files)_
- [x] **P3-03** Formula lexer/parser (A1/R1C1, structured refs, array constants, LAMBDA/LET).
- [x] **P3-04** Dependency graph + incremental recalculation, cycle detection, multithreaded calc. _(spatial bucket index, Kahn levels, rayon on native / single-threaded in WASM)_
- [x] **P3-05** Dynamic array evaluation & spill engine (`#SPILL!`, `#` spill-range operator).
- [x] **P3-06** Core function library (math, stat, text, logical, date/time, lookup). _(250 functions incl. 23 financial; engineering functions (BIN2DEC, CONVERT…) not included)_
- [x] **P3-07** XLOOKUP / XMATCH incl. regex match mode (`match_mode = 3`).
- [x] **P3-08** REGEXTEST / REGEXEXTRACT / REGEXREPLACE. _(fancy-regex: look-around, back-references, atomic groups; backtracking bounded)_
- [x] **P3-09** GROUPBY / PIVOTBY / PERCENTOF / TRIMRANGE. _(incl. `A1:.A10` / `A1.:A10` / `A1.:.A10` trim references)_
- [x] **P3-10** Virtualized grid canvas (frozen panes, merged cells, 1M+ rows smooth scroll). _(1,048,576 rows with scaled scrolling; 1M-row/2M-cell model: WASM load ≈4–5 s, ≈830 MB; viewport fetch <1 ms/frame; one edit + SUM(B:B) ≈90 ms in WASM, ≈37 ms native)_
- [x] **P3-11** Cell editing, formula bar, autocomplete, fill handle, selection model.
- [x] **P3-12** Python sandbox daemon (isolated subprocess, pandas/numpy/scipy/matplotlib). _(per-cell process; Linux bubblewrap (no network, read-only root, private /tmp and $HOME) → `unshare -rn` fallback; macOS sandbox-exec; Windows Job Object (memory cap, no child processes — network not blocked on Windows); uses the packages installed in the local Python)_
- [x] **P3-13** `=PY(expression, return_type)` — DataFrame spill & plot image output. _(async `#BUSY!`, `xl()` inputs, DataFrame/list spill, matplotlib figures drawn in the cell; pictures are not written into the .xlsx)_
- [x] **P3-14** Charts (DrawingML chartML) and pivot tables UI. _(6 chart types: insert, move, resize, retype, retitle, delete — also for charts from files; classic PivotTables saved with pivot caches (LibreOffice recognises and recomputes them identically) or live PIVOTBY formulas; pivots sharing a cache with another are passed through untouched)_
- [x] **P3-15** Data connections (CSV, JSON, SQL via ODBC) & refresh. _(CSV/JSON/SQLite/ODBC (pyodbc), Refresh All also refreshes PivotTables; saved in xl/connections.xml, foreign connections kept)_

## Phase 4 — Presentation & Vector Engine

- [x] **P4-01** Shared DrawingML engine crate (shapes, geometry presets, fills, gradients, effects, 3D). _(`prism-drawingml`: 99 preset geometries + custom geometry (guide evaluator, arcTo), theme colours with PowerPoint-exact transforms, solid/gradient/pattern/picture fills, lines with dashes/arrowheads, outer shadows, text bodies; Rust + WASM. 3D (`scene3d`/`sp3d`), glow, soft edges and reflection are preserved in files but not rendered; `sun` has no rays and callouts are approximations)_
- [x] **P4-02** PPTX import/export (slides, layouts, masters, themes, notes). _(`prism-slide`: lossless round-trip — unchanged shapes are written byte-identical; add/remove/reorder slides, layouts, notes (creates a notes master), backgrounds, hidden slides, pictures, tables, groups, transitions incl. Morph, animations; checked with python-pptx and LibreOffice. SmartArt/OLE/media frames pass through untouched)_
- [~] **P4-03** GPU slide compositor (WebGPU) with thumbnails rail and zoom. _(Canvas 2D slide rasteriser (placeholder inheritance, text layout with autofit, charts from cached values) composited by the shared P2-07 compositor — WebGL2 by default; the WebGPU backend is opt-in and untested on real GPUs, like P2-07)_
- [x] **P4-04** Slide editing: selection, transform handles, snapping, alignment guides. _(move/resize/rotate handles incl. rotated shapes, smart guides to slide and shapes, marquee, in-place rich text editing, insert text boxes/shapes/pictures/tables, z-order, group/ungroup, clipboard, notes, layouts, undo/redo)_
- [x] **P4-05** Animation timeline & transitions, incl. Morph. _(click-step timeline (on click / with / after previous, delays, repeats), entrance/exit/emphasis/motion paths, 17 transitions, Morph matching shapes by name/picture/text/placeholder with position/size/rotation/colour interpolation. Rare effects (blinds, checkerboard, strips…) play as fades; Morph by word/character crossfades text rather than moving letters)_
- [x] **P4-06** Slideshow mode + presenter view (dual display). _(full screen, builds, auto-advance, jump to slide, black/white screen; presenter view with current/next slide, notes, timer and an audience window placed on the second screen via the Window Management API when allowed. The audience window uses `window.open`; inside the Tauri shell this still needs verifying on real hardware (a native second window may be required))_
- [x] **P4-07** Cameo live camera object (vector-masked frames, effects, morphs across slides). _(camera feed masked by any preset geometry, border/shadow, mirror, device picker; Morph carries a cameo across slides by name. Stored as a normal shape + Prism extLst data, so PowerPoint shows its fill; PowerPoint's own cameos are not recognised as live)_
- [~] **P4-08** Native capture: AVFoundation (macOS) / Media Foundation (Windows). _(capture goes through the webview media stack — AVFoundation in WKWebView, Media Foundation in WebView2 — with macOS usage strings and hardened-runtime entitlements; there is no separate native Rust capture pipeline, and none of this was run on real macOS/Windows hardware here)_
- [x] **P4-09** Recording Studio: multi-track camera/mic/ink/transition timeline. _(narration, camera track, timestamped pen/highlighter/eraser ink, navigation timeline, pause/resume, preview, save timings as auto-advance. Takes live in memory and are exported as video — narration is not embedded into the .pptx)_
- [x] **P4-10** Teleprompter in presenter view (positioned under webcam). _(top-centre band in the presenter view and the studio: speed, size, mirror, voice-paced scrolling)_
- [x] **P4-11** MP4 export 1080p/4K with chapter bookmarks. _(offline frame rendering incl. transitions, Morph, builds, ink and recorded camera; WebCodecs H.264 (VP9 fallback) + AAC (Opus fallback); own ISO BMFF muxer with Nero `chpl` chapters — output verified with ffprobe/ffmpeg. Codec availability depends on the webview (Linux Chromium has no AAC encoder, so audio falls back to Opus))_
- [x] **P4-12** AI presentation generator (from text/bullets/linked docx). _(JSON outline from the AI daemon laid out on the deck's own layouts with speaker notes; Word documents are read through `prism-word` as Markdown. Not exercised against the live API in this environment (no key))_
- [x] **P4-13** Speaker Coach (pace, pitch, filler words, engagement). _(local: pitch + monotone detection, loudness, long pauses, syllable-based pace; with the webview's speech recognition also words/min, fillers (en/az/tr/ru), repetition and reading the slide. Speech recognition exists only in some webviews (and Chromium's is an online service); without it fillers are not counted. No camera-based eye-contact analysis)_
- [x] **P4-14** Live Presenting: QR audience link + live translated subtitles. _(token-protected LAN server in the host (viewer page, slide image, server-sent events), QR code from an own ISO 18004 encoder, subtitles from speech recognition translated with the AI service into each viewer's language. Same network only, plain HTTP (no TLS); not tried on real phones here — the server is covered by an integration test)_

## Cross-cutting

- [x] **X-01** Local-first auto-recovery: structural JSON-diff journal (splice ops, fsync) every 3 s per modified document, compaction, torn-write tolerance, Hub restore/discard; journals dropped on save or deliberate close.
- [x] **X-02** AI service daemon: provider trait (Claude Messages API), streaming, refusal fallbacks, retries with retry-after/exponential backoff, key in the OS keychain (0600 file fallback), privacy switch.
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

- **2026-11-13** — Phase 4 complete (two items partial): `prism-drawingml` and
  `prism-slide` crates (lossless PPTX), canvas slide renderer on the shared
  compositor, slide editor with handles/guides/rich text, animations,
  transitions and Morph, slideshow + presenter view, Cameo, Recording Studio,
  teleprompter, MP4 export with chapters, AI deck generator, Speaker Coach and
  Live Presenting. Also fixed the app CSP so the WASM engines may compile in
  WebView2/WKWebView (`wasm-unsafe-eval`).

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
