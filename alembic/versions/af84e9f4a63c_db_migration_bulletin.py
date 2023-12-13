"""db migration, bulletin

Revision ID: af84e9f4a63c
Revises: 5a8d5b637728
Create Date: 2023-12-13 19:44:55.144995

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "af84e9f4a63c"
down_revision: Union[str, None] = "5a8d5b637728"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.create_table(
        "bulletins",
        sa.Column("id", sa.String(length=256), nullable=False),
        sa.Column("title", sa.String(length=256), nullable=True),
        sa.Column("content", sa.Text(), nullable=True),
        sa.PrimaryKeyConstraint("id"),
    )
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_table("bulletins")
    # ### end Alembic commands ###