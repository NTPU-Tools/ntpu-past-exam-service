"""db migration, departments_courses

Revision ID: 79eb4c2e02f3
Revises: e7bae20f6ba2
Create Date: 2023-12-15 00:29:31.565985

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "79eb4c2e02f3"
down_revision: Union[str, None] = "e7bae20f6ba2"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "courses", sa.Column("department_id", sa.String(length=256), nullable=True)
    )
    op.drop_index("ix_courses_category", table_name="courses")
    op.drop_index("ix_courses_name", table_name="courses")
    op.create_index(
        op.f("ix_courses_department_id"), "courses", ["department_id"], unique=False
    )
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index(op.f("ix_courses_department_id"), table_name="courses")
    op.create_index("ix_courses_name", "courses", ["name"], unique=False)
    op.create_index("ix_courses_category", "courses", ["category"], unique=False)
    op.drop_column("courses", "department_id")
    # ### end Alembic commands ###