import os
import uuid

from dotenv import load_dotenv
from sqlalchemy import Column, DateTime, String, create_engine, func
from sqlalchemy.orm import declarative_base, sessionmaker

load_dotenv()


# pylint: disable-next=line-too-long
SQLALCHEMY_DATABASE_URL = f"mysql+mysqlconnector://{os.getenv('DATABASE_USERNAME')}:{os.getenv('DATABASE_PASSWORD')}@{os.getenv('DATABASE_HOST')}:{os.getenv('DATABASE_PORT')}/{os.getenv('DATABASE')}"

engine = create_engine(SQLALCHEMY_DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

Base = declarative_base()


def generate_uuid():
    return str(uuid.uuid4())


class BaseColumn(object):  # pylint: disable=useless-object-inheritance
    id = Column(String(256), primary_key=True, default=generate_uuid)
    create_time = Column(
        DateTime(timezone=True),
        server_default=func.now(),  # pylint: disable=not-callable
    )
    updated_time = Column(
        DateTime(timezone=True),
        onupdate=func.now(),  # pylint: disable=not-callable
    )


def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()
