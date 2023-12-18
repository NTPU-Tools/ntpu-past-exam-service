"""super user

Revision ID: c4251455152f
Revises: 4da81afb7b6c
Create Date: 2023-12-19 00:53:34.238162

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "c4251455152f"
down_revision: Union[str, None] = "4da81afb7b6c"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column("users", sa.Column("is_super_user", sa.Boolean(), nullable=True))
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column("users", "is_super_user")
    # ### end Alembic commands ###
