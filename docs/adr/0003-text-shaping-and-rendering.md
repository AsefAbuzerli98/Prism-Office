# 0003. Text shaping, fonts and page rendering

- **Status:** Accepted
- **Date:** 2026-11-12
- **Tasks:** P2-04, P2-05, P2-07

## Context

The specification lists rustybuzz/HarfBuzz shaping, font fallback and
embedded fonts (P2-05) and a WebGPU/WebGL page renderer (P2-07). Layout
(line breaking, justification, pagination) is ours (ADR-0001), but glyph
shaping and rasterisation could be done either by our own engine or by the
WebView's text stack.

## Decision

- **Shaping and rasterisation are delegated to the WebView text stack**
  (HarfBuzz/Skia in WebView2, CoreText in WKWebView, both behind Canvas 2D
  `measureText`/`fillText`). Layout measures with the same API it draws
  with, so metrics and pixels always agree; complex scripts (Arabic joining,
  Indic reordering, CJK, emoji) are shaped by the platform. A bundled
  rustybuzz-in-WASM shaper is **not** used.
- **Font fallback:** each run uses a CSS font stack — the document font,
  then a metric-compatible open substitute (Calibri→Carlito,
  Cambria→Caladea, Arial→Liberation Sans/Arimo, Times New Roman→Liberation
  Serif/Tinos, Courier New→Liberation Mono/Cousine, …), then a generic
  family; per-glyph fallback beyond that is the platform's.
- **Embedded fonts** in DOCX (`w:embedRegular` etc.) are de-obfuscated in
  Rust (ECMA-376 §17.8.1) and registered with the `FontFace` API before
  layout.
- **Line breaking** is a UAX #14 subset (spaces, hyphens, dashes, slashes,
  CJK, ZWSP, non-breaking glue, no break before closing / after opening
  punctuation) plus soft hyphens and Liang hyphenation (en-US patterns;
  Turkish patterns approximate Azerbaijani). **Bidi** reorders fragments
  per UAX #9 rule L2 with paragraph base direction from `w:bidi`.
- **Rendering:** pages are rasterised once per layout version into
  offscreen canvases and composited by WebGL2 (default) or Canvas 2D; a
  WebGPU compositor exists behind `localStorage["prism.compositor"]` until
  it is validated on real GPUs (it presented nothing under headless
  SwiftShader). Caret/selection live on a separate overlay layer.

## Alternatives considered

- **rustybuzz in WASM + our own glyph rasteriser/atlas:** byte-identical
  metrics on every OS, but megabytes of fonts/WASM, our own hinting and
  subpixel rendering, and a second source of truth that must be kept in
  sync with how the platform draws text in the rest of the UI.
- **Full-document Canvas 2D repaint per frame:** simplest, but repaints all
  visible text on every caret blink and scroll.

## Consequences

- Line breaks can differ slightly between macOS and Windows when fonts
  differ; this is the same trade-off Word makes across platforms.
  Metric-compatible fallbacks keep pagination close to Word's when Office
  fonts are missing.
- Full UAX #14 (e.g. Thai dictionary breaking) and Knuth–Plass paragraph
  optimisation remain future work.
