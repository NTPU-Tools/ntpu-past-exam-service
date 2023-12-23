"""public department

Revision ID: cfeaac9eed4a
Revises: c4251455152f
Create Date: 2023-12-22 08:53:40.130670

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "cfeaac9eed4a"
down_revision: Union[str, None] = "c4251455152f"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column("departments", sa.Column("is_public", sa.Boolean(), nullable=True))
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column("departments", "is_public")
    # ### end Alembic commands ###