from typing import Optional

from pydantic import BaseModel


class UserBase(BaseModel):
    username: str


class UserCreate(UserBase):
    hashed_password: str
    readable_name: str


class User(UserBase):
    id: str
    email: Optional[str]

    class Config:
        from_attributes = True


class UserPreferencesUpdate(BaseModel):
    show_empty_courses: bool
