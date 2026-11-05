#!/usr/bin/env python3
"""Check that every route of the reference implementation is exercised by the
contract suite.

Reads contract-tests/reference/routes.json (exported from the FastAPI app by
reference/export_routes.py) and scans contract-tests/tests/*.rs for HTTP calls
made through the harness (`ctx.get(...)`, `ctx.post(...)`, ...). Exits non-zero
when a route has no call site.

    python3 contract-tests/scripts/check_route_coverage.py
"""

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROUTES = json.loads((ROOT / "reference" / "routes.json").read_text())["routes"]

CALL_LITERAL = re.compile(
    r"\.(get|post|put|delete)\(\s*(?:&\s*)?(?:format!\(\s*)?\"(/[^\"]*)\"", re.S
)
CALL_VARIABLE = re.compile(r"\.(get|post|put|delete)\(\s*&?\s*(\w+)\s*\)", re.S)
ALL_PAGES = re.compile(r"get_all_pages\(\s*\"(/[^\"]*)\"", re.S)
ASSIGN = re.compile(r"let\s+(\w+)\s*=\s*format!\(\s*\"(/[^\"]*)\"", re.S)
CONST = re.compile(r"const\s+(\w+)\s*:\s*&str\s*=\s*\"(/[^\"]*)\"")
FOR_LIST = re.compile(r"for\s+(\w+)\s+in\s+\[([^\]]*)\]", re.S)
STRING = re.compile(r"\"([^\"]*)\"")
NAMED_PLACEHOLDER = re.compile(r"\{(\w+)\}")
PLACEHOLDER = re.compile(r"^\{[^}]*\}$")


def calls_in(text):
    """Resolve (method, path) pairs for every harness call in a test file.

    Variables are resolved to the nearest preceding `let x = format!("...")`
    or `for x in ["...", ...]`, and `{x}` placeholders bound by such a for-list
    are expanded to the listed literals.
    """
    bindings = []  # (position, name, [values])
    for m in ASSIGN.finditer(text):
        bindings.append((m.start(), m.group(1), [m.group(2)]))
    for m in FOR_LIST.finditer(text):
        values = STRING.findall(m.group(2))
        if values:
            bindings.append((m.start(), m.group(1), values))
    consts = {m.group(1): m.group(2) for m in CONST.finditer(text)}

    def resolve(name, position):
        best = None
        for pos, var, values in bindings:
            if var == name and pos < position and (best is None or pos > best[0]):
                best = (pos, values)
        if best:
            return best[1]
        return [consts[name]] if name in consts else []

    def expand(path, position):
        paths = [path]
        for name in set(NAMED_PLACEHOLDER.findall(path)):
            values = [v for v in resolve(name, position) if "/" not in v]
            if values:
                paths = [p.replace("{" + name + "}", v) for p in paths for v in values]
        return paths

    found = []
    for m in CALL_LITERAL.finditer(text):
        found += [(m.group(1).upper(), p) for p in expand(m.group(2), m.start())]
    for m in ALL_PAGES.finditer(text):
        found.append(("GET", m.group(1)))
    for m in CALL_VARIABLE.finditer(text):
        for path in resolve(m.group(2), m.start()):
            if path.startswith("/"):
                found += [(m.group(1).upper(), p) for p in expand(path, m.start())]
    return found


def match_score(route_path, test_path):
    route_segments = route_path.strip("/").split("/")
    test_segments = test_path.split("?")[0].strip("/").split("/")
    if len(route_segments) != len(test_segments):
        return None
    score = 0
    for r, t in zip(route_segments, test_segments):
        r_param = r.startswith("{")
        t_placeholder = bool(PLACEHOLDER.match(t))
        if r_param:
            continue
        if t_placeholder or r != t:
            return None
        score += 1
    return score


def main():
    coverage = defaultdict(list)
    unmatched = []
    for test_file in sorted((ROOT / "tests").glob("*.rs")):
        for method, path in calls_in(test_file.read_text()):
            candidates = []
            for route in ROUTES:
                if route["method"] != method:
                    continue
                score = match_score(route["path"], path)
                if score is not None:
                    candidates.append((score, route["path"]))
            if not candidates:
                unmatched.append((test_file.name, method, path))
                continue
            best = max(score for score, _ in candidates)
            for score, route_path in candidates:
                if score == best:
                    coverage[(method, route_path)].append(test_file.stem)

    missing = []
    print(f"{'route':58} call sites  test files")
    for route in ROUTES:
        key = (route["method"], route["path"])
        hits = coverage.get(key, [])
        files = ", ".join(sorted(set(hits)))
        print(f"{route['method'] + ' ' + route['path']:58} {len(hits):>10}  {files}")
        if not hits:
            missing.append(key)

    if unmatched:
        print("\ncalls that match no reference route (expected 404/405 probes):")
        for name, method, path in unmatched:
            print(f"  {name}: {method} {path}")

    print(f"\n{len(ROUTES) - len(missing)}/{len(ROUTES)} routes exercised")
    if missing:
        print("MISSING:", ", ".join(f"{m} {p}" for m, p in missing))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
