import uuid

from sqlalchemy import Boolean, Column, String

from sql.database import Base, BaseColumn


def generate_uuid():
    return str(uuid.uuid4())


class Department(Base, BaseColumn):
    __tablename__ = "departments"

    key = Column(String(256))
    name = Column(String(256))
    is_public = Column(Boolean, default=True, index=True)
