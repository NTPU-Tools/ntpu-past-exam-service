"""db migration, department admin remove user admin column

Revision ID: cf6012afa993
Revises: 0f40557ac560
Create Date: 2023-12-16 13:26:00.893588

"""
from typing import Sequence, Union

import sqlalchemy as sa
from sqlalchemy.dialects import mysql

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "cf6012afa993"
down_revision: Union[str, None] = "0f40557ac560"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column("users", "is_active")
    op.drop_column("users", "is_admin")
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "users",
        sa.Column(
            "is_admin",
            mysql.TINYINT(display_width=1),
            autoincrement=False,
            nullable=True,
        ),
    )
    op.add_column(
        "users",
        sa.Column(
            "is_active",
            mysql.TINYINT(display_width=1),
            autoincrement=False,
            nullable=True,
        ),
    )
    # ### end Alembic commands ###
