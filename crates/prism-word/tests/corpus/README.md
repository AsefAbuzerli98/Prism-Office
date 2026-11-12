# DOCX round-trip corpus

Real-world documents used by `tests/corpus.rs` (P2-03). Each file must read,
re-export without new validation errors, re-read to an identical model, keep
untouched parts byte-identical, and re-export idempotently.

| File(s) | Source | License |
| --- | --- | --- |
| `Bug51170.docx`, `Bug54771a.docx`, `SampleDoc.docx`, `TestDocument.docx`, `WithTabs.docx`, `comment.docx`, `footnotes.docx`, `headerFooter.docx`, `heading123.docx` | Apache POI `test-data/document` | Apache License 2.0 |
| `pydocx-test.docx`, `pydocx-having-images.docx` | python-docx `tests/test_files` | MIT |
