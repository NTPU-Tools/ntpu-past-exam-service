"""Write the Alembic revision chain that the Rust backend accepts at startup.

The Rust service refuses to start when the database is *behind* its build, so
it needs every known revision in order. `rust/schema_revisions.txt` lists them
oldest first; the last line is the head.

    python scripts/sync_schema_revisions.py          # rewrite the file
    python scripts/sync_schema_revisions.py --check  # exit 1 if it is stale
"""

import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSIONS = ROOT / "alembic" / "versions"
OUTPUT = ROOT / "rust" / "schema_revisions.txt"


def read_revision(path: Path) -> tuple[str, str | None]:
    values = {}
    for node in ast.parse(path.read_text()).body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                if isinstance(target, ast.Name) and target.id in (
                    "revision",
                    "down_revision",
                ):
                    values[target.id] = ast.literal_eval(node.value)
    down = values.get("down_revision")
    if isinstance(down, (tuple, list)):
        raise SystemExit(f"{path.name}: merge revisions are not supported")
    return values["revision"], down


def revision_chain() -> list[str]:
    parents = dict(read_revision(path) for path in VERSIONS.glob("*.py"))
    children: dict[str | None, list[str]] = {}
    for revision, down in parents.items():
        children.setdefault(down, []).append(revision)
    for down, revisions in children.items():
        if len(revisions) > 1:
            raise SystemExit(f"multiple heads branch from {down}: {sorted(revisions)}")

    chain = []
    current = None
    while current in children:
        current = children[current][0]
        chain.append(current)
    if len(chain) != len(parents):
        raise SystemExit("alembic history is not a single linear chain")
    return chain


def main() -> int:
    content = "\n".join(revision_chain()) + "\n"
    if "--check" in sys.argv[1:]:
        if not OUTPUT.exists() or OUTPUT.read_text() != content:
            print(
                f"{OUTPUT.relative_to(ROOT)} is stale; "
                "run python scripts/sync_schema_revisions.py",
                file=sys.stderr,
            )
            return 1
        return 0
    OUTPUT.write_text(content)
    return 0


if __name__ == "__main__":
    sys.exit(main())
