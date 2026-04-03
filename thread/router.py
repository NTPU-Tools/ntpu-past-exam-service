from typing import Annotated, Optional

from fastapi import APIRouter, Depends, File, Form, HTTPException, Request, UploadFile
from jose import JWTError
from sqlalchemy.orm import Session

from courses.models import Course
from sql.database import get_db
from users.models import User, UserDepartment
from utils.token import get_access_token_payload

from . import dependencies

router = APIRouter(prefix="/threads")

MAX_PAGE_LIMIT = 100
MAX_THREAD_IMAGE_SIZE_BYTES = 5 * 1024 * 1024
ALLOWED_THREAD_IMAGE_TYPES = {
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
}
_IMAGE_MAGIC_BYTES = {
    b"\xff\xd8\xff": "image/jpeg",
    b"\x89PNG\r\n\x1a\n": "image/png",
    b"GIF87a": "image/gif",
    b"GIF89a": "image/gif",
    b"RIFF": "image/webp",  # further checked below
}


def _validate_image_magic(data: bytes) -> bool:
    for magic, _ in _IMAGE_MAGIC_BYTES.items():
        if data.startswith(magic):
            if magic == b"RIFF":
                return len(data) >= 12 and data[8:12] == b"WEBP"
            return True
    return False


MAX_PAGE_SKIP = 10000


def _normalize_pagination(skip: int, limit: int):
    normalized_skip = min(max(skip, 0), MAX_PAGE_SKIP)
    normalized_limit = max(1, min(limit, MAX_PAGE_LIMIT))
    return normalized_skip, normalized_limit


@router.post("/{course_id}")
async def create_thread(
    course_id: str,
    title: Annotated[str, Form(min_length=1, max_length=256)],
    content: Annotated[str, Form(min_length=1, max_length=10000)],
    is_anonymous: Annotated[bool, Form()] = False,
    image: Annotated[Optional[UploadFile], File()] = None,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    course = db.query(Course).filter(Course.id == course_id).first()
    if course is None:
        raise HTTPException(status_code=404, detail="Course not found")

    image_data = None
    if image is not None:
        if image.content_type not in ALLOWED_THREAD_IMAGE_TYPES:
            raise HTTPException(status_code=400, detail="Unsupported image type")

        image_data = await image.read(MAX_THREAD_IMAGE_SIZE_BYTES + 1)
        if len(image_data) > MAX_THREAD_IMAGE_SIZE_BYTES:
            raise HTTPException(status_code=413, detail="Image too large")
        if not _validate_image_magic(image_data):
            raise HTTPException(status_code=400, detail="Invalid image content")

    thread_data = {
        "title": title,
        "content": content,
        "course_id": course_id,
        "is_anonymous": is_anonymous,
    }
    thread = dependencies.create_thread(db, thread_data, user_id, image_data)
    return {"status": "success", "thread_id": thread.id}


@router.get("/{course_id}")
def get_threads(
    course_id: str,
    skip: int = 0,
    limit: int = 100,
    request: Request = None,
    db: Session = Depends(get_db),
):
    skip, limit = _normalize_pagination(skip, limit)

    user_id = None
    try:
        payload = get_access_token_payload(request)
        user_id = payload.get("id")
    except (JWTError, HTTPException, KeyError, AttributeError):
        pass
    threads = dependencies.get_threads(db, course_id, skip, limit, user_id)
    return threads


@router.get("/detail/{thread_id}")
def get_thread_detail(
    thread_id: str,
    request: Request = None,
    db: Session = Depends(get_db),
):
    user_id = None
    try:
        payload = get_access_token_payload(request)
        user_id = payload.get("id")
    except (JWTError, HTTPException, KeyError, AttributeError):
        pass
    thread = dependencies.get_thread(db, thread_id, user_id)
    if thread is None:
        raise HTTPException(status_code=404, detail="Thread not found")
    return thread


@router.put("/{thread_id}")
def update_thread(
    thread_id: str,
    title: Annotated[Optional[str], Form(min_length=1, max_length=256)] = None,
    content: Annotated[Optional[str], Form(min_length=1, max_length=10000)] = None,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    thread = (
        db.query(dependencies.models.Thread)
        .filter(dependencies.models.Thread.id == thread_id)
        .first()
    )
    if thread is None:
        raise HTTPException(status_code=404, detail="Thread not found")
    if thread.owner_id != user_id:
        raise HTTPException(status_code=403, detail="Permission denied")

    update_data = {}
    if title is not None:
        update_data["title"] = title
    if content is not None:
        update_data["content"] = content

    thread = dependencies.update_thread(db, thread_id, update_data)
    return {"status": "success", "thread_id": thread.id}


@router.delete("/{thread_id}")
def delete_thread(
    thread_id: str,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")
    is_super_user = payload.get("isu", False)

    thread = (
        db.query(dependencies.models.Thread)
        .filter(dependencies.models.Thread.id == thread_id)
        .first()
    )

    if thread is None:
        raise HTTPException(status_code=404, detail="Thread not found")

    is_department_admin = False
    if not is_super_user:
        course = db.query(Course).filter(Course.id == thread.course_id).first()
        if course is not None:
            is_department_admin = (
                db.query(UserDepartment)
                .filter(
                    UserDepartment.user_id == user_id,
                    UserDepartment.department_id == course.department_id,
                    UserDepartment.status == "APPROVED",
                    UserDepartment.is_department_admin,
                )
                .first()
                is not None
            )

    can_delete = thread.owner_id == user_id or is_department_admin or is_super_user
    if not can_delete:
        raise HTTPException(status_code=403, detail="Permission denied")

    success = dependencies.delete_thread(db, thread)
    if success:
        return {"status": "success"}
    raise HTTPException(status_code=404, detail="Thread not found")


@router.post("/{thread_id}/like")
def like_thread(
    request: Request,
    thread_id: str,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    thread = dependencies.toggle_thread_like(db, thread_id, user_id)

    if thread is None:
        raise HTTPException(status_code=404, detail="Thread not found")

    return thread


@router.post("/{thread_id}/comments")
def create_comment(
    thread_id: str,
    content: Annotated[str, Form(min_length=1, max_length=10000)],
    is_anonymous: Annotated[bool, Form()] = False,
    parent_comment_id: Annotated[Optional[str], Form()] = None,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    thread = (
        db.query(dependencies.models.Thread)
        .filter(dependencies.models.Thread.id == thread_id)
        .first()
    )
    if thread is None:
        raise HTTPException(status_code=404, detail="Thread not found")

    parent_comment = None
    if parent_comment_id:
        parent_comment = (
            db.query(dependencies.models.ThreadComment)
            .filter(dependencies.models.ThreadComment.id == parent_comment_id)
            .first()
        )
        if parent_comment is None:
            raise HTTPException(status_code=404, detail="Parent comment not found")
        if parent_comment.thread_id != thread_id:
            raise HTTPException(status_code=400, detail="Parent comment does not belong to this thread")

    comment_data = {
        "content": content,
        "is_anonymous": is_anonymous,
    }

    comment = dependencies.create_comment(
        db, thread_id, comment_data, user_id, parent_comment_id
    )
    return {"status": "success", "comment_id": comment.id}


@router.get("/{thread_id}/comments")
def get_comments(
    thread_id: str,
    skip: int = 0,
    limit: int = 100,
    request: Request = None,
    db: Session = Depends(get_db),
):
    skip, limit = _normalize_pagination(skip, limit)

    user_id = None
    try:
        payload = get_access_token_payload(request)
        user_id = payload.get("id")
    except (JWTError, HTTPException, KeyError, AttributeError):
        pass
    comments = dependencies.get_comments(db, thread_id, skip, limit, user_id)
    return comments


@router.get("/comments/{comment_id}")
def get_comment_detail(
    comment_id: str,
    request: Request = None,
    db: Session = Depends(get_db),
):
    user_id = None
    try:
        payload = get_access_token_payload(request)
        user_id = payload.get("id")
    except (JWTError, HTTPException, KeyError, AttributeError):
        pass
    comment = dependencies.get_comment_with_replies(db, comment_id, user_id)
    if comment is None:
        raise HTTPException(status_code=404, detail="Comment not found")
    return comment


@router.put("/comments/{comment_id}")
def update_comment(
    comment_id: str,
    content: Annotated[Optional[str], Form(min_length=1, max_length=10000)] = None,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    comment = (
        db.query(dependencies.models.ThreadComment)
        .filter(dependencies.models.ThreadComment.id == comment_id)
        .first()
    )
    if comment is None:
        raise HTTPException(status_code=404, detail="Comment not found")
    if comment.owner_id != user_id:
        raise HTTPException(status_code=403, detail="Permission denied")

    if content is None:
        raise HTTPException(status_code=400, detail="Content is required")

    update_data = {"content": content}
    comment = dependencies.update_comment(db, comment_id, update_data)
    return {"status": "success", "comment_id": comment.id}


@router.delete("/comments/{comment_id}")
def delete_comment(
    comment_id: str,
    request: Request = None,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")
    is_super_user = payload.get("isu", False)

    comment = (
        db.query(dependencies.models.ThreadComment)
        .filter(dependencies.models.ThreadComment.id == comment_id)
        .first()
    )
    if comment is None:
        raise HTTPException(status_code=404, detail="Comment not found")

    is_department_admin = False
    if not is_super_user:
        thread = (
            db.query(dependencies.models.Thread)
            .filter(dependencies.models.Thread.id == comment.thread_id)
            .first()
        )
        if thread is not None:
            course = db.query(Course).filter(Course.id == thread.course_id).first()
            if course is not None:
                is_department_admin = (
                    db.query(UserDepartment)
                    .filter(
                        UserDepartment.user_id == user_id,
                        UserDepartment.department_id == course.department_id,
                        UserDepartment.status == "APPROVED",
                        UserDepartment.is_department_admin,
                    )
                    .first()
                    is not None
                )

    can_delete = comment.owner_id == user_id or is_department_admin or is_super_user
    if not can_delete:
        raise HTTPException(status_code=403, detail="Permission denied")

    success = dependencies.delete_comment(db, comment_id)
    if success:
        return {"status": "success"}
    raise HTTPException(status_code=404, detail="Comment not found")


@router.post("/comments/{comment_id}/like")
def comment_like(
    request: Request,
    comment_id: str,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    if not user_id:
        raise HTTPException(status_code=401, detail="Invalid token: missing user id")

    comment = dependencies.toggle_comment_like(db, comment_id, user_id)
    if comment is None:
        raise HTTPException(status_code=404, detail="Comment not found")
    return comment
