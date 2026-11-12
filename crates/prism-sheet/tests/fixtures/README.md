# XLSX fixtures

Generated for this project (no third-party content):

- `openpyxl.xlsx` — `python3 make_fixtures.py openpyxl.xlsx` (openpyxl 3.1):
  shared strings, number/date formats, formulas incl. `_xlfn.XLOOKUP`, a
  defined name, a table, data validation, a comment, merged cells, frozen
  panes and column widths.
- `libreoffice.xlsx` — the same file re-saved by LibreOffice 24.2
  (`soffice --headless --convert-to xlsx`), which writes a different but
  equivalent structure (verbose sheet views, `<cols>` attributes, …).
