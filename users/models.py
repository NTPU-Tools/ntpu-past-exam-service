from sqlalchemy import Boolean, Column, ForeignKey, String, Text

from sql.database import Base, BaseColumn


class User(Base, BaseColumn):
    __tablename__ = "users"

    email = Column(String(256))
    username = Column(String(256), unique=True)
    readable_name = Column(String(256), nullable=True, default=None)
    note = Column(Text(), nullable=True, default=None)
    hashed_password = Column(String(256), nullable=True)
    school_department = Column(String(256))
    is_super_user = Column(Boolean, default=False)


class UserPreference(Base, BaseColumn):
    __tablename__ = "user_preferences"

    user_id = Column(
        String(256), ForeignKey("users.id", ondelete="CASCADE"), unique=True, index=True, nullable=False
    )
    show_empty_courses = Column(Boolean, default=True, nullable=False)


class UserDepartment(Base, BaseColumn):
    __tablename__ = "users_departments"

    user_id = Column(String(256), ForeignKey("users.id", ondelete="CASCADE"), index=True)
    department_id = Column(String(256), ForeignKey("departments.id", ondelete="CASCADE"), index=True)
    status = Column(String(256), default="PENDING", index=True)
    is_department_admin = Column(Boolean, default=False, index=True)
