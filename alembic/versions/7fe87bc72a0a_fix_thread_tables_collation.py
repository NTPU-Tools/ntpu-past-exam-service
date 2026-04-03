"""fix_thread_tables_collation

Revision ID: 7fe87bc72a0a
Revises: a93158ec4674
Create Date: 2026-03-15 13:50:08.285710

"""

from typing import Sequence, Union

from alembic import op


# revision identifiers, used by Alembic.
revision: str = "7fe87bc72a0a"
down_revision: Union[str, None] = "a93158ec4674"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.execute(
        "ALTER TABLE threads CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci"
    )
    op.execute(
        "ALTER TABLE thread_comments CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci"
    )


def downgrade() -> None:
    op.execute(
        "ALTER TABLE threads CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )
    op.execute(
        "ALTER TABLE thread_comments CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    )
