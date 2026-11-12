"""Prism Office =PY runner. Reads one JSON request on stdin, writes one JSON
result on stdout. Runs under `python3 -I` in a throw-away directory.

The sandbox is best-effort (no OS isolation): resource limits, no network
sockets, no subprocesses, and file writes only inside the working dir.
"""
import ast, builtins, io, json, math, os, sys

req = json.loads(sys.stdin.read())
real_stdout = sys.stdout

try:
    import resource
    mem = int(req.get("memoryMb", 512)) * 1024 * 1024
    for lim in (resource.RLIMIT_AS,):
        try:
            resource.setrlimit(lim, (mem, mem))
        except (ValueError, OSError):
            pass
    cpu = int(req.get("cpuSeconds", 20))
    resource.setrlimit(resource.RLIMIT_CPU, (cpu, cpu + 1))
except ImportError:  # Windows: the host enforces the wall-clock timeout
    pass

try:
    import pandas as pd  # optional, like Excel's Anaconda environment
except Exception:  # noqa: BLE001
    pd = None
try:
    import numpy as np
except Exception:  # noqa: BLE001
    np = None

INPUTS = {k: v for k, v in req.get("inputs", [])}


def _cell(v):
    if isinstance(v, dict) and "error" in v:
        return float("nan")
    return v


def xl(ref, headers=False):
    """Values of a referenced range: a DataFrame when pandas is available."""
    if ref not in INPUTS:
        raise KeyError(f"xl(): {ref!r} was not passed to this cell")
    rows = [[_cell(c) for c in r] for r in INPUTS[ref]]
    if len(rows) == 1 and len(rows[0]) == 1 and not headers:
        return rows[0][0]
    if pd is not None:
        if headers and rows:
            return pd.DataFrame(rows[1:], columns=[str(c) for c in rows[0]])
        return pd.DataFrame(rows)
    return rows


# ---- sandbox ------------------------------------------------------------------------------
import socket  # noqa: E402

def _blocked(*_a, **_k):
    raise PermissionError("network access is disabled in Python cells")

socket.socket = _blocked  # type: ignore[assignment]
socket.create_connection = _blocked  # type: ignore[assignment]
for name in ("system", "popen", "fork", "forkpty", "execv", "execve", "execvp", "spawnv", "spawnve", "kill"):
    if hasattr(os, name):
        setattr(os, name, _blocked)
try:
    import subprocess
    subprocess.Popen = _blocked  # type: ignore[assignment]
    subprocess.run = _blocked  # type: ignore[assignment]
except ImportError:
    pass

_open = builtins.open
_root = os.path.realpath(os.getcwd())

def _guarded_open(file, mode="r", *a, **k):
    if any(m in mode for m in "wax+"):
        p = os.path.realpath(os.fspath(file))
        if not p.startswith(_root + os.sep):
            raise PermissionError("Python cells may only write inside their scratch folder")
    return _open(file, mode, *a, **k)

builtins.open = _guarded_open

# ---- run ----------------------------------------------------------------------------------

def to_json(v, excel_values):
    if pd is not None and isinstance(v, pd.DataFrame):
        if not excel_values:
            return f"DataFrame {v.shape[0]}×{v.shape[1]}"
        head = [str(c) for c in v.columns]
        body = [[to_scalar(x) for x in row] for row in v.itertuples(index=False)]
        return [head] + body if not all(isinstance(c, int) for c in v.columns) else body
    if pd is not None and isinstance(v, pd.Series):
        return [[to_scalar(x)] for x in v.tolist()] if excel_values else f"Series ({len(v)})"
    if np is not None and isinstance(v, np.ndarray):
        v = v.tolist()
    if isinstance(v, (list, tuple)):
        # lists always spill: rows of rows, or a single column
        if v and all(isinstance(r, (list, tuple)) for r in v):
            return [[to_scalar(x) for x in r] for r in v]
        return [[to_scalar(x)] for x in v]
    return to_scalar(v)


def to_scalar(x):
    if np is not None and isinstance(x, np.generic):
        x = x.item()
    if x is None:
        return None
    if isinstance(x, bool):
        return x
    if isinstance(x, (int, float)):
        if isinstance(x, float) and (math.isnan(x) or math.isinf(x)):
            return {"e": "#NUM!"}
        return x
    return str(x)


out = io.StringIO()
result = {"value": None, "stdout": "", "error": None}
try:
    tree = ast.parse(req["code"], "<cell>", "exec")
    last = None
    if tree.body and isinstance(tree.body[-1], ast.Expr):
        last = ast.Expression(tree.body.pop().value)
    env = {"xl": xl, "__name__": "__main__"}
    if pd is not None:
        env["pd"] = pd
    if np is not None:
        env["np"] = np
    sys.stdout = out
    exec(compile(tree, "<cell>", "exec"), env)
    value = eval(compile(last, "<cell>", "eval"), env) if last else None
    sys.stdout = real_stdout
    result["value"] = to_json(value, int(req.get("returnType", 0)) == 1)
except BaseException as e:  # noqa: BLE001
    sys.stdout = real_stdout
    result["error"] = f"{type(e).__name__}: {e}"
result["stdout"] = out.getvalue()[-4000:]
real_stdout.write(json.dumps(result))
