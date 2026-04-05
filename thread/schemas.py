from datetime import datetime
from typing import List, Optional

from pydantic import BaseModel


class ThreadResponse(BaseModel):
    id: str
    title: str
    content: str
    image_url: Optional[str] = None
    course_id: str
    is_anonymous: bool
    like_count: int
    create_time: datetime
    updated_time: Optional[datetime] = None
    owner_id: str
    owner_name: str
    is_owner: bool
    liked: Optional[bool] = None


class ThreadListResponse(BaseModel):
    threads: List[ThreadResponse]
    total: int


class ThreadCreateResponse(BaseModel):
    status: str
    thread_id: str


class ThreadUpdateResponse(BaseModel):
    status: str
    thread_id: str


class ToggleThreadLikeResponse(BaseModel):
    thread: ThreadResponse
    liked: bool


class CommentResponse(BaseModel):
    id: str
    thread_id: str
    parent_comment_id: Optional[str] = None
    content: str
    is_anonymous: bool
    like_count: int
    create_time: datetime
    updated_time: Optional[datetime] = None
    owner_id: str
    owner_name: str
    is_owner: bool
    liked: Optional[bool] = None
    reply_count: Optional[int] = None
    replies: Optional[List["CommentResponse"]] = None


CommentResponse.model_rebuild()


class CommentCreateResponse(BaseModel):
    status: str
    comment_id: str


class CommentUpdateResponse(BaseModel):
    status: str
    comment_id: str


class ToggleCommentLikeResponse(BaseModel):
    comment: CommentResponse
    liked: bool


class DeleteResponse(BaseModel):
    status: str
