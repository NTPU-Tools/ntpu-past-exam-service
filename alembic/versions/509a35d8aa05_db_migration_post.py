"""db migration, post

Revision ID: 509a35d8aa05
Revises: c019a1a57efc
Create Date: 2023-11-30 01:20:19.073923

"""
from typing import Sequence, Union

import sqlalchemy as sa
from sqlalchemy.dialects import mysql

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "509a35d8aa05"
down_revision: Union[str, None] = "c019a1a57efc"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column("posts", sa.Column("content", sa.Text(), nullable=True))
    op.drop_column("posts", "description")
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column("posts", sa.Column("description", mysql.TEXT(), nullable=True))
    op.drop_column("posts", "content")
    # ### end Alembic commands ###
