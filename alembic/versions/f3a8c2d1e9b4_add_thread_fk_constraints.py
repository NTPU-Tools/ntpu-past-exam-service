"""add foreign key constraints to thread tables

Revision ID: f3a8c2d1e9b4
Revises: b1c2d3e4f5a6
Create Date: 2026-04-05 00:00:00.000000

"""

from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "f3a8c2d1e9b4"
down_revision: Union[str, None] = "b1c2d3e4f5a6"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    op.create_foreign_key(
        "fk_threads_owner_id", "threads", "users", ["owner_id"], ["id"]
    )
    op.create_foreign_key(
        "fk_threads_course_id", "threads", "courses", ["course_id"], ["id"]
    )
    op.create_foreign_key(
        "fk_thread_comments_thread_id",
        "thread_comments",
        "threads",
        ["thread_id"],
        ["id"],
    )
    op.create_foreign_key(
        "fk_thread_comments_parent_comment_id",
        "thread_comments",
        "thread_comments",
        ["parent_comment_id"],
        ["id"],
    )
    op.create_foreign_key(
        "fk_thread_comments_owner_id",
        "thread_comments",
        "users",
        ["owner_id"],
        ["id"],
    )
    op.create_foreign_key(
        "fk_thread_likes_thread_id", "thread_likes", "threads", ["thread_id"], ["id"]
    )
    op.create_foreign_key(
        "fk_thread_likes_user_id", "thread_likes", "users", ["user_id"], ["id"]
    )
    op.create_foreign_key(
        "fk_comment_likes_comment_id",
        "comment_likes",
        "thread_comments",
        ["comment_id"],
        ["id"],
    )
    op.create_foreign_key(
        "fk_comment_likes_user_id", "comment_likes", "users", ["user_id"], ["id"]
    )


def downgrade() -> None:
    op.drop_constraint("fk_comment_likes_user_id", "comment_likes", type_="foreignkey")
    op.drop_constraint(
        "fk_comment_likes_comment_id", "comment_likes", type_="foreignkey"
    )
    op.drop_constraint("fk_thread_likes_user_id", "thread_likes", type_="foreignkey")
    op.drop_constraint("fk_thread_likes_thread_id", "thread_likes", type_="foreignkey")
    op.drop_constraint(
        "fk_thread_comments_owner_id", "thread_comments", type_="foreignkey"
    )
    op.drop_constraint(
        "fk_thread_comments_parent_comment_id", "thread_comments", type_="foreignkey"
    )
    op.drop_constraint(
        "fk_thread_comments_thread_id", "thread_comments", type_="foreignkey"
    )
    op.drop_constraint("fk_threads_course_id", "threads", type_="foreignkey")
    op.drop_constraint("fk_threads_owner_id", "threads", type_="foreignkey")
