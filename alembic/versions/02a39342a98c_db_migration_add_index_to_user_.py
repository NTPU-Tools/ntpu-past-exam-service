"""db migration, add index to user department

Revision ID: 02a39342a98c
Revises: cf6012afa993
Create Date: 2023-12-16 16:32:44.540972

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "02a39342a98c"
down_revision: Union[str, None] = "cf6012afa993"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index("ix_posts_title", table_name="posts")
    op.create_index(op.f("ix_posts_course_id"), "posts", ["course_id"], unique=False)
    op.create_index(op.f("ix_posts_owner_id"), "posts", ["owner_id"], unique=False)
    op.create_index(
        op.f("ix_users_departments_department_id"),
        "users_departments",
        ["department_id"],
        unique=False,
    )
    op.create_index(
        op.f("ix_users_departments_user_id"),
        "users_departments",
        ["user_id"],
        unique=False,
    )
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index(op.f("ix_users_departments_user_id"), table_name="users_departments")
    op.drop_index(
        op.f("ix_users_departments_department_id"), table_name="users_departments"
    )
    op.drop_index(op.f("ix_posts_owner_id"), table_name="posts")
    op.drop_index(op.f("ix_posts_course_id"), table_name="posts")
    op.create_index("ix_posts_title", "posts", ["title"], unique=False)
    # ### end Alembic commands ###
