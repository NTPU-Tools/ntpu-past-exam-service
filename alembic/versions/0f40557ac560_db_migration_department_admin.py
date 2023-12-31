"""db migration, department admin

Revision ID: 0f40557ac560
Revises: 56947cc2d6e0
Create Date: 2023-12-16 12:14:05.034199

"""
from typing import Sequence, Union

import sqlalchemy as sa
from sqlalchemy.dialects import mysql

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "0f40557ac560"
down_revision: Union[str, None] = "56947cc2d6e0"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "users_departments",
        sa.Column("is_department_admin", sa.Boolean(), nullable=True),
    )
    op.drop_column("users_departments", "is_admin")
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "users_departments",
        sa.Column(
            "is_admin",
            mysql.TINYINT(display_width=1),
            autoincrement=False,
            nullable=True,
        ),
    )
    op.drop_column("users_departments", "is_department_admin")
    # ### end Alembic commands ###
