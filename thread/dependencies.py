import logging
import os
import uuid
from collections import defaultdict
from typing import Dict, Optional, Tuple

from dotenv import load_dotenv
from sqlalchemy import func
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from static_file.r2 import r2
from users.models import User

from . import models

logger = logging.getLogger(__name__)

load_dotenv()


def _serialize_thread(thread: models.Thread, user: User, liked: Optional[bool] = None, current_user_id: Optional[str] = None):
    data = {
        "id": thread.id,
        "title": thread.title,
        "content": thread.content,
        "image_url": thread.image_url,
        "course_id": thread.course_id,
        "is_anonymous": thread.is_anonymous,
        "like_count": thread.like_count or 0,
        "create_time": thread.create_time,
        "updated_time": thread.updated_time,
        "owner_id": "anonymous" if thread.is_anonymous else thread.owner_id,
        "owner_name": (
            "匿名用戶"
            if thread.is_anonymous
            else (user.readable_name if user.readable_name else user.username)
        ),
        "is_owner": (current_user_id == thread.owner_id) if current_user_id else False,
    }
    if liked is not None:
        data["liked"] = liked
    return data


def _serialize_comment(
    comment: models.ThreadComment,
    user: User,
    liked: Optional[bool] = None,
    reply_count: Optional[int] = None,
    current_user_id: Optional[str] = None,
):
    data = {
        "id": comment.id,
        "thread_id": comment.thread_id,
        "parent_comment_id": comment.parent_comment_id,
        "content": comment.content,
        "is_anonymous": comment.is_anonymous,
        "like_count": comment.like_count or 0,
        "create_time": comment.create_time,
        "updated_time": comment.updated_time,
        "owner_id": "anonymous" if comment.is_anonymous else comment.owner_id,
        "owner_name": (
            "匿名用戶"
            if comment.is_anonymous
            else (user.readable_name if user.readable_name else user.username)
        ),
        "is_owner": (current_user_id == comment.owner_id) if current_user_id else False,
    }
    if liked is not None:
        data["liked"] = liked
    if reply_count is not None:
        data["reply_count"] = reply_count
    return data


def get_threads(
    db: Session, course_id: str, skip: int = 0, limit: int = 100, user_id: str = None
) -> Tuple[list, int]:
    base_filter = models.Thread.course_id == course_id
    total = db.query(func.count(models.Thread.id)).filter(base_filter).scalar()

    query_result = (
        db.query(models.Thread, User)
        .filter(base_filter)
        .join(User, models.Thread.owner_id == User.id)
        .order_by(models.Thread.create_time.desc())
        .offset(skip)
        .limit(limit)
        .all()
    )

    if not query_result:
        return [], total

    liked_thread_ids = set()
    if user_id:
        thread_ids = [thread.id for thread, _ in query_result]
        liked_thread_ids = {
            thread_id
            for (thread_id,) in db.query(models.ThreadLike.thread_id)
            .filter(
                models.ThreadLike.thread_id.in_(thread_ids),
                models.ThreadLike.user_id == user_id,
            )
            .all()
        }

    threads = [
        _serialize_thread(thread, user, liked=thread.id in liked_thread_ids, current_user_id=user_id)
        for thread, user in query_result
    ]
    return threads, total


def get_thread(db: Session, thread_id: str, user_id: str = None):
    query_result = (
        db.query(models.Thread, User)
        .filter(models.Thread.id == thread_id)
        .join(User, models.Thread.owner_id == User.id)
        .first()
    )

    if query_result is None:
        return None

    thread, user = query_result

    liked = None
    if user_id:
        liked = (
            db.query(models.ThreadLike)
            .filter(
                models.ThreadLike.thread_id == thread_id,
                models.ThreadLike.user_id == user_id,
            )
            .first()
            is not None
        )

    return _serialize_thread(thread, user, liked=liked, current_user_id=user_id)


def create_thread(
    db: Session, thread_data: Dict, user_id: str, image_data: Optional[bytes] = None
):
    key = None
    image_url = None
    if image_data is not None:
        key = f"threads/{str(uuid.uuid4())}"
        image_url = f"{os.getenv('R2_FILE_PATH')}/{key}"

    db_thread = models.Thread(
        **thread_data,
        owner_id=user_id,
        image_url=image_url,
    )
    db.add(db_thread)
    db.commit()
    db.refresh(db_thread)

    if image_data is not None:
        try:
            r2.put_object(Body=image_data, Bucket=os.getenv("R2_BUCKET_NAME"), Key=key)
        except Exception:
            db.delete(db_thread)
            db.commit()
            raise

    return db_thread


def update_thread(db: Session, thread_id: str, update_data: Dict):
    db_thread = db.query(models.Thread).filter(models.Thread.id == thread_id).first()
    if db_thread is None:
        return None

    for key, value in update_data.items():
        if key in ["title", "content"]:
            setattr(db_thread, key, value)

    db.commit()
    db.refresh(db_thread)
    return db_thread


def delete_thread(db: Session, db_thread: models.Thread):
    comment_ids = (
        db.query(models.ThreadComment.id)
        .filter(models.ThreadComment.thread_id == db_thread.id)
        .all()
    )
    comment_ids = [comment_id for (comment_id,) in comment_ids]

    if comment_ids:
        db.query(models.CommentLike).filter(
            models.CommentLike.comment_id.in_(comment_ids)
        ).delete(synchronize_session=False)
        # Delete child comments before top-level ones to satisfy the
        # self-referential FK on parent_comment_id.
        db.query(models.ThreadComment).filter(
            models.ThreadComment.thread_id == db_thread.id,
            models.ThreadComment.parent_comment_id.isnot(None),
        ).delete(synchronize_session=False)
        db.query(models.ThreadComment).filter(
            models.ThreadComment.thread_id == db_thread.id,
        ).delete(synchronize_session=False)

    db.query(models.ThreadLike).filter(
        models.ThreadLike.thread_id == db_thread.id
    ).delete(synchronize_session=False)
    if db_thread.image_url:
        key = db_thread.image_url.replace(f"{os.getenv('R2_FILE_PATH')}/", "", 1)
        try:
            r2.delete_object(Bucket=os.getenv("R2_BUCKET_NAME"), Key=key)
        except Exception:
            logger.warning("Failed to delete R2 object for thread %s (key=%s)", db_thread.id, key, exc_info=True)

    db.delete(db_thread)
    db.commit()
    return True


def get_comments(
    db: Session, thread_id: str, skip: int = 0, limit: int = 100, user_id: str = None
):
    query_result = (
        db.query(models.ThreadComment, User)
        .filter(
            models.ThreadComment.thread_id == thread_id,
            models.ThreadComment.parent_comment_id.is_(None),
        )
        .join(User, models.ThreadComment.owner_id == User.id)
        .order_by(models.ThreadComment.create_time.asc())
        .offset(skip)
        .limit(limit)
        .all()
    )

    if not query_result:
        return []

    comment_ids = [comment.id for comment, _ in query_result]

    reply_count_rows = (
        db.query(
            models.ThreadComment.parent_comment_id,
            func.count(models.ThreadComment.id),
        )
        .filter(models.ThreadComment.parent_comment_id.in_(comment_ids))
        .group_by(models.ThreadComment.parent_comment_id)
        .all()
    )
    reply_count_map = {
        parent_comment_id: reply_count
        for parent_comment_id, reply_count in reply_count_rows
    }

    liked_comment_ids = set()
    if user_id:
        liked_comment_ids = {
            comment_id
            for (comment_id,) in db.query(models.CommentLike.comment_id)
            .filter(
                models.CommentLike.comment_id.in_(comment_ids),
                models.CommentLike.user_id == user_id,
            )
            .all()
        }

    comments = []
    for comment, user in query_result:
        comments.append(
            _serialize_comment(
                comment,
                user,
                liked=comment.id in liked_comment_ids,
                reply_count=reply_count_map.get(comment.id, 0),
                current_user_id=user_id,
            )
        )
    return comments


def get_comment_with_replies(db: Session, comment_id: str, user_id: str = None):
    query_result = (
        db.query(models.ThreadComment, User)
        .filter(models.ThreadComment.id == comment_id)
        .join(User, models.ThreadComment.owner_id == User.id)
        .first()
    )

    if query_result is None:
        return None

    comment, user = query_result

    MAX_REPLY_DEPTH = 10
    MAX_TOTAL_REPLIES = 500

    replies_by_parent_id = defaultdict(list)
    current_parent_ids = [comment_id]
    seen_comment_ids = {comment_id}
    all_comments = [(comment, user)]
    depth = 0

    while current_parent_ids and depth < MAX_REPLY_DEPTH and len(all_comments) < MAX_TOTAL_REPLIES:
        reply_results = (
            db.query(models.ThreadComment, User)
            .filter(models.ThreadComment.parent_comment_id.in_(current_parent_ids))
            .join(User, models.ThreadComment.owner_id == User.id)
            .order_by(models.ThreadComment.create_time.asc())
            .all()
        )

        if not reply_results:
            break

        next_parent_ids = []
        for reply, reply_user in reply_results:
            if reply.id in seen_comment_ids:
                continue
            seen_comment_ids.add(reply.id)
            replies_by_parent_id[reply.parent_comment_id].append((reply, reply_user))
            all_comments.append((reply, reply_user))
            next_parent_ids.append(reply.id)

        current_parent_ids = next_parent_ids
        depth += 1

    liked_comment_ids = set()
    if user_id:
        all_comment_ids = [comment_item.id for comment_item, _ in all_comments]
        liked_comment_ids = {
            liked_comment_id
            for (liked_comment_id,) in db.query(models.CommentLike.comment_id)
            .filter(
                models.CommentLike.comment_id.in_(all_comment_ids),
                models.CommentLike.user_id == user_id,
            )
            .all()
        }

    reply_count_map = {pid: len(replies) for pid, replies in replies_by_parent_id.items()}

    comment_data_map = {}
    for comment_item, comment_user in all_comments:
        comment_data = _serialize_comment(
            comment_item,
            comment_user,
            liked=comment_item.id in liked_comment_ids,
            reply_count=reply_count_map.get(comment_item.id, 0),
            current_user_id=user_id,
        )
        comment_data["replies"] = []
        comment_data_map[comment_item.id] = comment_data

    for parent_id, replies in replies_by_parent_id.items():
        parent_data = comment_data_map.get(parent_id)
        if parent_data is None:
            continue
        for reply, _ in replies:
            parent_data["replies"].append(comment_data_map[reply.id])

    return comment_data_map[comment_id]


def create_comment(
    db: Session,
    thread_id: str,
    comment_data: Dict,
    user_id: str,
    parent_comment_id: Optional[str] = None,
):
    db_comment = models.ThreadComment(
        thread_id=thread_id,
        parent_comment_id=parent_comment_id,
        owner_id=user_id,
        **comment_data,
    )
    db.add(db_comment)
    db.commit()
    db.refresh(db_comment)
    return db_comment


def update_comment(db: Session, comment_id: str, update_data: Dict):
    db_comment = (
        db.query(models.ThreadComment)
        .filter(models.ThreadComment.id == comment_id)
        .first()
    )
    if db_comment is None:
        return None

    if "content" in update_data:
        db_comment.content = update_data["content"]

    db.commit()
    db.refresh(db_comment)
    return db_comment


_MAX_COMMENT_DELETE_IDS = 10_000


def delete_comment(db: Session, comment_id: str):
    comment_exists = (
        db.query(models.ThreadComment.id)
        .filter(models.ThreadComment.id == comment_id)
        .first()
    )
    if comment_exists is None:
        return False

    comment_ids = [comment_id]
    seen_comment_ids = {comment_id}
    cursor = 0

    while cursor < len(comment_ids) and len(comment_ids) < _MAX_COMMENT_DELETE_IDS:
        parent_ids = comment_ids[cursor : cursor + 100]
        cursor += len(parent_ids)

        child_ids = [
            child_id
            for (child_id,) in db.query(models.ThreadComment.id)
            .filter(models.ThreadComment.parent_comment_id.in_(parent_ids))
            .limit(_MAX_COMMENT_DELETE_IDS - len(comment_ids))
            .all()
        ]

        for child_id in child_ids:
            if child_id in seen_comment_ids:
                continue
            seen_comment_ids.add(child_id)
            comment_ids.append(child_id)

    db.query(models.CommentLike).filter(
        models.CommentLike.comment_id.in_(comment_ids)
    ).delete(synchronize_session=False)
    # Delete child comments before parents to satisfy the self-referential FK.
    db.query(models.ThreadComment).filter(
        models.ThreadComment.id.in_(comment_ids),
        models.ThreadComment.parent_comment_id.isnot(None),
    ).delete(synchronize_session=False)
    db.query(models.ThreadComment).filter(
        models.ThreadComment.id.in_(comment_ids),
    ).delete(synchronize_session=False)

    db.commit()
    return True


def toggle_thread_like(db: Session, thread_id: str, user_id: str):
    db_thread = (
        db.query(models.Thread)
        .filter(models.Thread.id == thread_id)
        .with_for_update()
        .first()
    )
    if db_thread is None:
        return None

    like = (
        db.query(models.ThreadLike)
        .filter(
            models.ThreadLike.thread_id == thread_id,
            models.ThreadLike.user_id == user_id,
        )
        .with_for_update()
        .first()
    )

    current_like_count = db_thread.like_count or 0

    if like is not None:
        db.delete(like)
        db_thread.like_count = max(current_like_count - 1, 0)
        liked = False
    else:
        db.add(models.ThreadLike(thread_id=thread_id, user_id=user_id))
        db_thread.like_count = current_like_count + 1
        liked = True

    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        db_thread = (
            db.query(models.Thread).filter(models.Thread.id == thread_id).first()
        )
        if db_thread is None:
            return None
        liked = (
            db.query(models.ThreadLike)
            .filter(
                models.ThreadLike.thread_id == thread_id,
                models.ThreadLike.user_id == user_id,
            )
            .first()
            is not None
        )
    else:
        db.refresh(db_thread)

    thread_user = db.query(User).filter(User.id == db_thread.owner_id).first()
    return {"thread": _serialize_thread(db_thread, thread_user, liked=liked, current_user_id=user_id), "liked": liked}


def toggle_comment_like(db: Session, comment_id: str, user_id: str):
    db_comment = (
        db.query(models.ThreadComment)
        .filter(models.ThreadComment.id == comment_id)
        .with_for_update()
        .first()
    )
    if db_comment is None:
        return None

    like = (
        db.query(models.CommentLike)
        .filter(
            models.CommentLike.comment_id == comment_id,
            models.CommentLike.user_id == user_id,
        )
        .with_for_update()
        .first()
    )

    current_like_count = db_comment.like_count or 0

    if like is not None:
        db.delete(like)
        db_comment.like_count = max(current_like_count - 1, 0)
        liked = False
    else:
        db.add(models.CommentLike(comment_id=comment_id, user_id=user_id))
        db_comment.like_count = current_like_count + 1
        liked = True

    try:
        db.commit()
    except IntegrityError:
        db.rollback()
        db_comment = (
            db.query(models.ThreadComment)
            .filter(models.ThreadComment.id == comment_id)
            .first()
        )
        if db_comment is None:
            return None
        liked = (
            db.query(models.CommentLike)
            .filter(
                models.CommentLike.comment_id == comment_id,
                models.CommentLike.user_id == user_id,
            )
            .first()
            is not None
        )
    else:
        db.refresh(db_comment)

    comment_user = db.query(User).filter(User.id == db_comment.owner_id).first()
    return {"comment": _serialize_comment(db_comment, comment_user, liked=liked, current_user_id=user_id), "liked": liked}
