"""add cascade delete to thread foreign keys

Revision ID: a1b2c3d4e5f6
Revises: f3a8c2d1e9b4
Create Date: 2026-04-06 00:00:00.000000

"""

from typing import Sequence, Union

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "a1b2c3d4e5f6"
down_revision: Union[str, None] = "f3a8c2d1e9b4"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # thread_comments.thread_id → threads.id  ON DELETE CASCADE
    op.drop_constraint("fk_thread_comments_thread_id", "thread_comments", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_comments_thread_id",
        "thread_comments",
        "threads",
        ["thread_id"],
        ["id"],
        ondelete="CASCADE",
    )

    # thread_comments.parent_comment_id → thread_comments.id  ON DELETE CASCADE
    op.drop_constraint("fk_thread_comments_parent_comment_id", "thread_comments", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_comments_parent_comment_id",
        "thread_comments",
        "thread_comments",
        ["parent_comment_id"],
        ["id"],
        ondelete="CASCADE",
    )

    # thread_likes.thread_id → threads.id  ON DELETE CASCADE
    op.drop_constraint("fk_thread_likes_thread_id", "thread_likes", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_likes_thread_id",
        "thread_likes",
        "threads",
        ["thread_id"],
        ["id"],
        ondelete="CASCADE",
    )

    # comment_likes.comment_id → thread_comments.id  ON DELETE CASCADE
    op.drop_constraint("fk_comment_likes_comment_id", "comment_likes", type_="foreignkey")
    op.create_foreign_key(
        "fk_comment_likes_comment_id",
        "comment_likes",
        "thread_comments",
        ["comment_id"],
        ["id"],
        ondelete="CASCADE",
    )


def downgrade() -> None:
    op.drop_constraint("fk_comment_likes_comment_id", "comment_likes", type_="foreignkey")
    op.create_foreign_key(
        "fk_comment_likes_comment_id",
        "comment_likes",
        "thread_comments",
        ["comment_id"],
        ["id"],
    )

    op.drop_constraint("fk_thread_likes_thread_id", "thread_likes", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_likes_thread_id",
        "thread_likes",
        "threads",
        ["thread_id"],
        ["id"],
    )

    op.drop_constraint("fk_thread_comments_parent_comment_id", "thread_comments", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_comments_parent_comment_id",
        "thread_comments",
        "thread_comments",
        ["parent_comment_id"],
        ["id"],
    )

    op.drop_constraint("fk_thread_comments_thread_id", "thread_comments", type_="foreignkey")
    op.create_foreign_key(
        "fk_thread_comments_thread_id",
        "thread_comments",
        "threads",
        ["thread_id"],
        ["id"],
    )
