"""db migration, post is migrate

Revision ID: 7b089676aa69
Revises: 65c84c08c11f
Create Date: 2023-12-08 11:05:58.454226

"""
from typing import Sequence, Union

import sqlalchemy as sa
from sqlalchemy.dialects import mysql

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "7b089676aa69"
down_revision: Union[str, None] = "65c84c08c11f"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index("ix_items_description", table_name="items")
    op.drop_index("ix_items_id", table_name="items")
    op.drop_index("ix_items_title", table_name="items")
    op.drop_table("items")
    op.add_column("posts", sa.Column("is_migrate", sa.Boolean(), nullable=True))
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column("posts", "is_migrate")
    op.create_table(
        "items",
        sa.Column("id", mysql.INTEGER(), autoincrement=True, nullable=False),
        sa.Column("title", mysql.VARCHAR(length=256), nullable=True),
        sa.Column("description", mysql.VARCHAR(length=256), nullable=True),
        sa.Column("owner_id", mysql.INTEGER(), autoincrement=False, nullable=True),
        sa.PrimaryKeyConstraint("id"),
        mysql_collate="utf8mb4_0900_ai_ci",
        mysql_default_charset="utf8mb4",
        mysql_engine="InnoDB",
    )
    op.create_index("ix_items_title", "items", ["title"], unique=False)
    op.create_index("ix_items_id", "items", ["id"], unique=False)
    op.create_index("ix_items_description", "items", ["description"], unique=False)
    # ### end Alembic commands ###