"""Create the PostgreSQL schema the same way production was bootstrapped.

The historical Alembic chain contains MySQL-only operations, so a fresh
PostgreSQL database is created from SQLAlchemy metadata and stamped at head
(mirrors scripts/seed.py:run_migrations).
"""

import os
import sys

sys.path.insert(0, "/app")

from alembic import command  # noqa: E402
from alembic.config import Config  # noqa: E402
from sqlalchemy import inspect  # noqa: E402

import sql.models  # noqa: E402,F401  # register every model
from sql.database import Base, engine  # noqa: E402


def main():
    if inspect(engine).get_table_names():
        print("schema already present")
        return
    Base.metadata.create_all(bind=engine)
    cfg = Config("/app/alembic.ini")
    cfg.set_main_option("script_location", "/app/alembic")
    command.stamp(cfg, "head")
    print("schema created and stamped")


if __name__ == "__main__":
    main()
    sys.stdout.flush()
    os._exit(0)
