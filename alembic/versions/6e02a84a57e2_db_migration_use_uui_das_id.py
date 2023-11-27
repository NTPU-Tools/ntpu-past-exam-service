"""db migration, use uui das id

Revision ID: 6e02a84a57e2
Revises: 41c05f3eb95f
Create Date: 2023-11-28 02:42:11.153620

"""
from typing import Sequence, Union

import sqlalchemy as sa
from sqlalchemy.dialects import mysql

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "6e02a84a57e2"
down_revision: Union[str, None] = "41c05f3eb95f"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column("items", sa.Column("uuid", sa.String(length=256), nullable=False))
    op.alter_column(
        "items",
        "owner_id",
        existing_type=mysql.INTEGER(),
        type_=sa.String(length=256),
        existing_nullable=True,
    )
    op.drop_index("ix_items_id", table_name="items")
    op.drop_column("items", "id")
    op.add_column("users", sa.Column("uuid", sa.String(length=256), nullable=False))
    op.drop_index("ix_users_id", table_name="users")
    op.drop_column("users", "id")
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column(
        "users", sa.Column("id", mysql.INTEGER(), autoincrement=True, nullable=False)
    )
    op.create_index("ix_users_id", "users", ["id"], unique=False)
    op.drop_column("users", "uuid")
    op.add_column(
        "items", sa.Column("id", mysql.INTEGER(), autoincrement=True, nullable=False)
    )
    op.create_index("ix_items_id", "items", ["id"], unique=False)
    op.alter_column(
        "items",
        "owner_id",
        existing_type=sa.String(length=256),
        type_=mysql.INTEGER(),
        existing_nullable=True,
    )
    op.drop_column("items", "uuid")
    # ### end Alembic commands ###