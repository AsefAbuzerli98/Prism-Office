# 0002. Lossless OpenXML editing via modelled + opaque content

- **Status:** Accepted
- **Date:** 2026-11-12
- **Tasks:** P1-10…P1-17, P2-01…P2-03

## Context

ISO/IEC 29500 is vast. The editors will support a growing subset of it, yet
opening and saving a file produced by Microsoft 365, Google Workspace or
iWork must never silently drop content we do not understand yet.

## Decision

- The OPC layer keeps every part as raw bytes (`Bytes`), so untouched parts
  round-trip byte-for-byte; only parts an editor changed are rewritten.
- Editor models represent supported constructs structurally and carry
  everything else verbatim:
  - unsupported blocks/inlines become `Opaque { label, xml }`;
  - unknown properties become `extra: [{ name, xml }]` lists;
  - pre-body content and root namespace declarations are reused from the
    original part on export.
- On export, known and preserved properties are merged in schema order
  (`order.rs`), because Word rejects out-of-order children.
- Definitions the editor needs (built-in list and paragraph styles) are added
  to `numbering.xml`/`styles.xml` only when referenced.

## Alternatives considered

- **Full schema-typed object model** (e.g. generated from XSD): complete but
  enormous, slow to compile and still lossy for extension namespaces.
- **DOM editing of the XML tree:** lossless, but couples the editor to XML
  details and makes layout/editing code far harder.

## Consequences

- Opaque content is visible as placeholders and cannot be edited until it is
  modelled; it survives save.
- Round-trip tests (`crates/prism-word/tests/roundtrip.rs`) assert model
  stability, schema order and idempotent re-export; a real-world corpus
  (X-04) is still needed.
