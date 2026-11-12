# 0001. Desktop stack: Tauri v2 + Rust core + TypeScript canvas UI

- **Status:** Accepted
- **Date:** 2026-11-12
- **Tasks:** P0-01, P1-01

## Context

Prism Office must ship on macOS (Apple Silicon, Intel) and Windows (x64,
ARM64) with a small memory footprint, native-feeling windows, identical
rendering across platforms, and fast, safe parsing of large untrusted
OpenXML files. Three editors (document, workbook, presentation) share one
shell, and several background services (AI, Python sandbox, recovery) must
be shared across windows.

## Decision

- **Native host:** Tauri v2 with a Rust backend. Rust owns file I/O, OPC
  packaging, OpenXML parsing/serialisation, window lifecycle, OS integration
  (dialogs, file associations, single instance) and shared daemons.
- **Core libraries as plain Rust crates** (`prism-openxml`, `prism-word`, …),
  independent of Tauri so they are unit-testable and reusable (CLI, fuzzing).
- **UI:** TypeScript (strict) without a component framework, rendering editor
  surfaces to `<canvas>` (2D now, WebGPU with WebGL2 fallback later) so
  layout and drawing are identical across WebView2 and WKWebView.
- **IPC:** typed Tauri commands for requests, one broadcast event channel
  (`prism://event`) for host → UI notifications.
- **Browser fallback:** an in-memory `BrowserHost` implements the same bridge
  so the UI runs and is tested without the native shell.

## Alternatives considered

- **Electron:** mature, one Chromium everywhere, but ~150 MB+ baseline memory
  per window and a Node backend that is a poor fit for zero-copy parsing.
- **Fully native per-platform UI (AppKit/WinUI):** best platform fidelity but
  three editors × two platforms of UI code and divergent text rendering.
- **Flutter / Qt:** single codebase, but weaker web-tech ecosystem for rich
  text tooling and a larger shift for contributors.
- **React/Vue for the shell:** convenient, but editor surfaces are canvas
  anyway; the remaining chrome is small enough that a framework adds more
  weight than value. Can be revisited for complex panels.

## Consequences

- Two WebView engines must be tested (WKWebView, WebView2); canvas rendering
  limits the differences to font rasterisation.
- Text layout is our responsibility (P2-04/P2-05) — browser layout is not used
  for documents.
- Linux builds work for development and CI (WebKitGTK) though not a release
  target.
- Platform-specific window code is behind `cfg(target_os)`; CI runs the Rust
  workspace on Linux, macOS and Windows to keep it compiling.
