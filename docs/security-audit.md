# Security audit (P5-05)

Scope: the Tauri host (`src-tauri`), its IPC commands, the webview content
policy, the file parsers (`prism-openxml`, `prism-word`, `prism-sheet`,
`prism-drawingml`, `prism-slide`), embedded code (`=PY`), external data
(data connections), the AI daemon and the Live Presenting server.

Threat model: a user opens a document from an untrusted source. The
document must not be able to run code outside a sandbox, read or write
files, reach the network, or crash/hang the app. Secondary: a bug that lets
document content run script in the webview must not turn into file-system
access through IPC.

Date: 2026-11-13. Status per finding: **fixed**, **mitigated** or **open**.

## Findings

| # | Area | Finding | Severity | Status |
|---|------|---------|----------|--------|
| 1 | Embedded code | `=PY` cells in a workbook opened from disk ran automatically on load. The sandbox blocks network/files on Linux (bubblewrap) and macOS (sandbox-exec) but on Windows the Job Object only caps memory and child processes. | High | **Fixed** — per-document consent before any `=PY` runs for files from disk; documents created in the session are trusted. Windows still has no network/file isolation (see open items). |
| 2 | External data | "Refresh All" read files / ran SQL against sources named in the workbook (`xl/connections.xml`) without saying so. | Medium | **Fixed** — first refresh of a file-from-disk lists every source and asks. |
| 3 | IPC | `write_binary_file(path, bytes)` let the page write any path. | High (with script injection) | **Fixed** — replaced by `export_file`: the save dialog runs in the host, the page only suggests a file name; extension forced. |
| 4 | IPC | `read_text_file(path)` read any file (≤ 64 MB). | Medium | **Fixed** — only `csv/tsv/txt/json/md` regular files. |
| 5 | Parsers | Stack overflow (process abort, not catchable) on deeply nested input: formula parentheses/unary chains, DrawingML group nesting, Word nested tables, any XML part parsed by the DrawingML DOM. Found with the fuzz harness and targeted probes. | High (DoS on open) | **Fixed** — formula length ≤ 8,192 and nesting ≤ 128 (Excel: 8,192 / 64); groups deeper than 32 kept as opaque verbatim nodes; tables nested > 64 rejected; DOM depth ≤ 256. Regression tests run on 2 MB test-thread stacks. |
| 6 | Parsers | Panics on malformed input: number formats with a decimal point after the exponent (`00E-0.00`), non-ASCII after `<`/`>` in formulas. | Medium | **Fixed**, inputs kept in `crates/prism-fuzz/regressions/`. |
| 6b | Calculation | Hang: fraction formats with many denominator digits (`?/???????????`) searched every denominator (10¹¹ steps; integer overflow beyond 18 digits). Stack overflow: `INDIRECT` evaluated text such as `A1:INDIRECT(C3)` that called itself without end. | High (DoS on open) | **Fixed** — continued-fraction best approximation (O(log n)); INDIRECT accepts plain references only and bounds name chains. |
| 7 | Live server | One thread per connection, unbounded; no security headers; token travels in the URL. | Low | **Fixed** — ≤ 300 concurrent connections (503 beyond), `Referrer-Policy: no-referrer`, CSP, `X-Frame-Options: DENY`, `nosniff`. 128-bit random token, GET only, no file system paths are served. Plain HTTP on the LAN remains (see open items). |
| 8 | Recovery journals | Journals hold document content but were created with the default umask. | Low | **Fixed** — journal directories `0700` on Unix. |
| 9 | Webview CSP | `default-src 'self'` without `wasm-unsafe-eval` blocked the WASM engines in WebView2/WKWebView. | Functional | **Fixed** — `script-src 'self' 'wasm-unsafe-eval'`; no remote origins, no `unsafe-eval`, `connect-src` limited to IPC. `style-src 'unsafe-inline'` remains (inline styles are used throughout). |

## Reviewed and acceptable

- **Zip bombs / huge parts**: `prism-openxml::Limits` caps part count and
  uncompressed part size before inflating; the fuzz harness runs with
  tighter limits.
- **XML external entities**: quick-xml never resolves external entities or
  DTDs; general entity references are limited to the predefined/numeric ones.
- **Path traversal in relationships**: part names are normalised by
  `PartName`; relationship targets outside the package are not followed.
  External targets (hyperlinks, linked images) are kept as data and never
  fetched.
- **HTML clipboard**: pasted HTML is parsed with `DOMParser` (inert document);
  nothing from documents is assigned to `innerHTML`. The only `innerHTML`
  uses insert QR-code SVG generated from numeric module data.
- **AI daemon**: the API key lives in the OS keychain (owner-only file
  fallback) and never reaches the page; the privacy switch blocks every
  request; document content is only sent when the user invokes a feature.
- **Capabilities**: windows `main` / `doc-*` get `core:default` and the
  dialog permissions only; no shell, fs or http plugins are enabled.
- **Saving documents**: `save_document` writes only through the registry
  and checks the extension matches the document kind.

## Fuzzing

`crates/prism-fuzz` defines eight targets (OPC container, DOCX, XLSX, PPTX,
formula text, number formats, DrawingML XML, preset geometry). The stable
runner mutates XML parts inside the ZIP so inputs pass CRC checks;
`fuzz/` holds the libFuzzer (cargo-fuzz, nightly) harness for the same
targets. See `fuzz/README.md`.

## Open items

1. **Windows Python isolation**: run `=PY` in an AppContainer (no network,
   no user-profile access) instead of a plain Job Object.
2. **Live Presenting over plain HTTP**: anyone on the same network who
   sees the URL can follow the slides. Options: TLS with a self-signed
   certificate pinned via the QR code, or a relay service.
3. **Formula evaluation limits**: recursive LAMBDA depth and very large
   array results are bounded by the engine's own guards but not yet by a
   wall-clock budget per recalculation.
4. **Code signing / notarization** (P5-03) are prerequisites for the
   updater's trust chain; until release keys exist the updater stays off.
5. External penetration test of the packaged apps on macOS and Windows.
