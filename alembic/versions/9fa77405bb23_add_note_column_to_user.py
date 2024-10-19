"""add note column to user

Revision ID: 9fa77405bb23
Revises: de2df7639c82
Create Date: 2024-10-20 00:30:00.705552

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op



# revision identifiers, used by Alembic.
revision: str = '9fa77405bb23'
down_revision: Union[str, None] = 'de2df7639c82'
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.add_column('users', sa.Column('note', sa.Text(), nullable=True))
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_column('users', 'note')
    # ### end Alembic commands ###
