"""Read-only database query for Prism data connections.
stdin: {"kind": "sqlite"|"odbc", "source": path-or-connection-string, "query": sql, "limit": n}
stdout: {"columns": [...], "rows": [[...]], "truncated": bool} or {"error": "..."}"""
import json, sys

req = json.loads(sys.stdin.read())
limit = int(req.get("limit", 100000))
try:
    kind = req["kind"]
    if kind == "sqlite":
        import sqlite3, pathlib
        uri = pathlib.Path(req["source"]).resolve().as_uri() + "?mode=ro"
        con = sqlite3.connect(uri, uri=True)
    elif kind == "odbc":
        try:
            import pyodbc  # type: ignore
        except ImportError:
            raise RuntimeError("ODBC needs the pyodbc package (pip install pyodbc) and a configured driver")
        con = pyodbc.connect(req["source"], readonly=True, autocommit=True)
    else:
        raise ValueError(f"unknown connection kind {kind!r}")
    cur = con.cursor()
    cur.execute(req["query"])
    cols = [d[0] for d in (cur.description or [])]
    rows = cur.fetchmany(limit + 1)
    def cell(v):
        if v is None or isinstance(v, (bool, int, float, str)):
            return v
        if isinstance(v, (bytes, bytearray, memoryview)):
            return f"<{len(bytes(v))} bytes>"
        return str(v)
    out = {"columns": cols, "rows": [[cell(v) for v in r] for r in rows[:limit]], "truncated": len(rows) > limit}
    con.close()
except Exception as e:  # noqa: BLE001
    out = {"error": f"{type(e).__name__}: {e}"}
sys.stdout.write(json.dumps(out))
