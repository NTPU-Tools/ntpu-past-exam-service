"""db migration, post

Revision ID: c019a1a57efc
Revises: 26d05dc9a63d
Create Date: 2023-11-29 20:15:06.046948

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "c019a1a57efc"
down_revision: Union[str, None] = "26d05dc9a63d"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    pass
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    pass
    # ### end Alembic commands ###
