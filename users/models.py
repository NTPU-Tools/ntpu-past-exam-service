import uuid

from sqlalchemy import Boolean, Column, String

from sql.database import Base


def generate_uuid():
    return str(uuid.uuid4())


class User(Base):
    __tablename__ = "users"

    id = Column(String(256), primary_key=True, default=generate_uuid)
    email = Column(String(256))
    username = Column(String(256), unique=True)
    readable_name = Column(String(256), nullable=True, default=None)
    hashed_password = Column(String(256))
    is_active = Column(Boolean, default=False)
    is_admin = Column(Boolean, default=False)
