"""Export the reference implementation's route table to contract-tests/reference/routes.json.

    docker compose -f contract-tests/docker-compose.yml run --rm --no-deps \
      --user "$(id -u)" -v "$PWD/contract-tests:/contract" schema \
      python /contract/reference/export_routes.py /contract/reference/routes.json
"""

import json
import os
import sys

sys.path.insert(0, "/app")
os.chdir("/app")

from fastapi.routing import APIRoute  # noqa: E402

from main import app  # noqa: E402

routes = []
for route in app.routes:
    if not isinstance(route, APIRoute):
        continue
    for method in sorted(route.methods):
        routes.append(
            {
                "method": method,
                "path": route.path,
                "endpoint": f"{route.endpoint.__module__}.{route.endpoint.__name__}",
            }
        )

with open(sys.argv[1], "w", encoding="utf-8") as handle:
    json.dump({"count": len(routes), "routes": routes}, handle, indent=2)
    handle.write("\n")
print(f"exported {len(routes)} routes")
sys.stdout.flush()
os._exit(0)
