"""add thread tables

Revision ID: a93158ec4674
Revises: 9fa77405bb23
Create Date: 2026-03-15 21:44:38.228314

"""

from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op


# revision identifiers, used by Alembic.
revision: str = "a93158ec4674"
down_revision: Union[str, None] = "9fa77405bb23"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    conn = op.get_bind()
    inspector = sa.inspect(conn)
    tables = inspector.get_table_names()

    if "threads" not in tables:
        op.create_table(
            "threads",
            sa.Column("id", sa.String(length=256), nullable=False),
            sa.Column("title", sa.String(length=256), nullable=True),
            sa.Column("content", sa.Text(), nullable=True),
            sa.Column("image_url", sa.String(length=1024), nullable=True),
            sa.Column("owner_id", sa.String(length=256), nullable=True),
            sa.Column("course_id", sa.String(length=256), nullable=True),
            sa.Column("is_anonymous", sa.Boolean(), nullable=True),
            sa.Column("like_count", sa.Integer(), nullable=True),
            sa.Column(
                "create_time",
                sa.DateTime(timezone=True),
                server_default=sa.text("now()"),
                nullable=True,
            ),
            sa.Column("updated_time", sa.DateTime(timezone=True), nullable=True),
            sa.PrimaryKeyConstraint("id"),
            mysql_charset="utf8mb4",
            mysql_collate="utf8mb4_unicode_ci",
        )
        op.create_index(
            op.f("ix_threads_owner_id"), "threads", ["owner_id"], unique=False
        )
        op.create_index(
            op.f("ix_threads_course_id"), "threads", ["course_id"], unique=False
        )

    if "thread_comments" not in tables:
        op.create_table(
            "thread_comments",
            sa.Column("id", sa.String(length=256), nullable=False),
            sa.Column("thread_id", sa.String(length=256), nullable=True),
            sa.Column("parent_comment_id", sa.String(length=256), nullable=True),
            sa.Column("content", sa.Text(), nullable=True),
            sa.Column("owner_id", sa.String(length=256), nullable=True),
            sa.Column("reply_to_user_id", sa.String(length=256), nullable=True),
            sa.Column("is_anonymous", sa.Boolean(), nullable=True),
            sa.Column("like_count", sa.Integer(), nullable=True),
            sa.Column(
                "create_time",
                sa.DateTime(timezone=True),
                server_default=sa.text("now()"),
                nullable=True,
            ),
            sa.Column("updated_time", sa.DateTime(timezone=True), nullable=True),
            sa.PrimaryKeyConstraint("id"),
            mysql_charset="utf8mb4",
            mysql_collate="utf8mb4_unicode_ci",
        )
        op.create_index(
            op.f("ix_thread_comments_thread_id"),
            "thread_comments",
            ["thread_id"],
            unique=False,
        )
        op.create_index(
            op.f("ix_thread_comments_parent_comment_id"),
            "thread_comments",
            ["parent_comment_id"],
            unique=False,
        )
        op.create_index(
            op.f("ix_thread_comments_owner_id"),
            "thread_comments",
            ["owner_id"],
            unique=False,
        )
        op.create_index(
            op.f("ix_thread_comments_reply_to_user_id"),
            "thread_comments",
            ["reply_to_user_id"],
            unique=False,
        )

    if "thread_likes" not in tables:
        op.create_table(
            "thread_likes",
            sa.Column("id", sa.String(length=256), nullable=False),
            sa.Column("thread_id", sa.String(length=256), nullable=True),
            sa.Column("user_id", sa.String(length=256), nullable=True),
            sa.Column(
                "create_time",
                sa.DateTime(timezone=True),
                server_default=sa.text("now()"),
                nullable=True,
            ),
            sa.Column("updated_time", sa.DateTime(timezone=True), nullable=True),
            sa.PrimaryKeyConstraint("id"),
            sa.UniqueConstraint("thread_id", "user_id", name="unique_thread_like"),
        )
        op.create_index(
            op.f("ix_thread_likes_thread_id"),
            "thread_likes",
            ["thread_id"],
            unique=False,
        )
        op.create_index(
            op.f("ix_thread_likes_user_id"), "thread_likes", ["user_id"], unique=False
        )

    if "comment_likes" not in tables:
        op.create_table(
            "comment_likes",
            sa.Column("id", sa.String(length=256), nullable=False),
            sa.Column("comment_id", sa.String(length=256), nullable=True),
            sa.Column("user_id", sa.String(length=256), nullable=True),
            sa.Column(
                "create_time",
                sa.DateTime(timezone=True),
                server_default=sa.text("now()"),
                nullable=True,
            ),
            sa.Column("updated_time", sa.DateTime(timezone=True), nullable=True),
            sa.PrimaryKeyConstraint("id"),
            sa.UniqueConstraint("comment_id", "user_id", name="unique_comment_like"),
        )
        op.create_index(
            op.f("ix_comment_likes_comment_id"),
            "comment_likes",
            ["comment_id"],
            unique=False,
        )
        op.create_index(
            op.f("ix_comment_likes_user_id"), "comment_likes", ["user_id"], unique=False
        )


def downgrade() -> None:
    op.drop_table("comment_likes")
    op.drop_table("thread_likes")
    op.drop_table("thread_comments")
    op.drop_table("threads")
