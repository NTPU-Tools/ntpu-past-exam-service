"""db migration, username

Revision ID: 3e0d6835822e
Revises: 9412f4be82f5
Create Date: 2023-11-29 10:54:01.551475

"""
from typing import Sequence, Union

import sqlalchemy as sa

from alembic import op

# revision identifiers, used by Alembic.
revision: str = "3e0d6835822e"
down_revision: Union[str, None] = "9412f4be82f5"
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index("ix_users_email", table_name="users")
    op.create_index(op.f("ix_users_username"), "users", ["username"], unique=True)
    # ### end Alembic commands ###


def downgrade() -> None:
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_index(op.f("ix_users_username"), table_name="users")
    op.create_index("ix_users_email", "users", ["email"], unique=False)
    # ### end Alembic commands ###
