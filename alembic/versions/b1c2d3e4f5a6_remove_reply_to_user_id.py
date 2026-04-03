"""remove reply_to_user_id from thread_comments

Revision ID: b1c2d3e4f5a6
Revises: 7fe87bc72a0a
Create Date: 2026-03-27 00:00:00.000000

"""

from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "b1c2d3e4f5a6"
down_revision: Union[str, None] = "7fe87bc72a0a"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.drop_index(
        op.f("ix_thread_comments_reply_to_user_id"),
        table_name="thread_comments",
    )
    op.drop_column("thread_comments", "reply_to_user_id")


def downgrade() -> None:
    op.add_column(
        "thread_comments",
        sa.Column("reply_to_user_id", sa.String(length=256), nullable=True),
    )
    op.create_index(
        op.f("ix_thread_comments_reply_to_user_id"),
        "thread_comments",
        ["reply_to_user_id"],
        unique=False,
    )
