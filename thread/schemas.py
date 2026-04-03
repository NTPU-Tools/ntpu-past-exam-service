from datetime import datetime
from typing import Optional

from pydantic import BaseModel, Field


class ThreadBase(BaseModel):
    title: str = Field(..., min_length=1, max_length=256)
    content: str = Field(..., min_length=1, max_length=10000)
    course_id: str
    is_anonymous: bool = False
    image_url: Optional[str] = None


class ThreadCreate(ThreadBase):
    pass


class ThreadUpdate(BaseModel):
    title: Optional[str] = None
    content: Optional[str] = None
    is_anonymous: Optional[bool] = None


class Thread(ThreadBase):
    id: str
    owner_id: str
    owner_name: str
    like_count: int
    create_time: datetime
    updated_time: datetime
    image_url: Optional[str] = None

    class Config:
        from_attributes = True


class ThreadCommentBase(BaseModel):
    content: str = Field(..., min_length=1, max_length=10000)
    is_anonymous: bool = False


class ThreadCommentCreate(ThreadCommentBase):
    parent_comment_id: Optional[str] = None


class ThreadCommentUpdate(BaseModel):
    content: Optional[str] = None


class ThreadComment(ThreadCommentBase):
    id: str
    thread_id: str
    parent_comment_id: Optional[str] = None
    owner_id: str
    owner_name: str
    like_count: int
    create_time: datetime
    updated_time: datetime

    class Config:
        from_attributes = True


class ThreadCommentDetail(ThreadComment):
    replies: Optional[list["ThreadComment"]] = None


class ThreadLikeResponse(BaseModel):
    thread: Thread
    liked: bool


class CommentLikeResponse(BaseModel):
    comment: ThreadComment
    liked: bool
