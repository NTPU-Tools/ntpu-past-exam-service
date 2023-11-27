import uuid

from sqlalchemy import Boolean, Column, String

from .database import Base


def generate_uuid():
    return str(uuid.uuid4())


class User(Base):
    __tablename__ = "users"

    id = Column(String(256), primary_key=True, default=generate_uuid)
    email = Column(String(256), unique=True, index=True)
    hashed_password = Column(String(256))
    is_active = Column(Boolean, default=True)


class Item(Base):
    __tablename__ = "items"

    id = Column(String(256), primary_key=True, default=generate_uuid)
    title = Column(String(256), index=True)
    description = Column(String(256), index=True)
    owner_id = Column(String(256))