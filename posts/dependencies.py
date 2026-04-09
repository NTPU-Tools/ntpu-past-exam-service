import os
import uuid
from typing import Dict, List

from dotenv import load_dotenv
from fastapi import HTTPException
from sqlalchemy.orm import Session

from courses.models import Course
from departments.dependencies import (
    get_department_admins,
    get_department_information,
    get_viewable_departments_ids,
)
from static_file.r2 import r2
from users.models import User
from utils.send_mail import send_notification_mail

from . import models

load_dotenv()

domain = os.getenv("ORIGIN")


def get_posts(
    db: Session,
    current_user_id: str,
    status: str,
    user_id: str,
    department_id: str,
    course_id: str,
    skip: int = 0,
    limit: int = 100,
):
    query_filter = []
    visible_department_ids = get_viewable_departments_ids(db, current_user_id)

    if not visible_department_ids:
        return []

    query_filter.append(models.Post.department_id.in_(visible_department_ids))

    if status == "PENDING":
        query_filter.append(
            (models.Post.status != "APPROVED") | (models.Post.status == None)
        )
    elif status == "APPROVED":
        query_filter.append(models.Post.status == "APPROVED")

    if course_id:
        query_filter.append(models.Post.course_id == course_id)

    if user_id:
        query_filter.append(models.Post.owner_id == user_id)

    if department_id:
        query_filter.append(models.Post.department_id == department_id)

    query_result = (
        db.query(models.Post, Course)
        .filter(*query_filter)
        .join(Course, models.Post.course_id == Course.id)
        .offset(skip)
        .limit(limit)
        .all()
    )
    posts = []
    for post, course in query_result:
        data = {
            **post.__dict__,
            "course_name": course.name,
            "owner_id": 'anonymous' if post.is_anonymous else post.owner_id
        }
        posts.append(data)
    return posts


def get_post(db: Session, post_id: str, current_user_id: str):
    query_result = (
        db.query(models.Post, User)
        .filter(models.Post.id == post_id)
        .join(User, models.Post.owner_id == User.id)
        .first()
    )
    if query_result is None:
        return None

    (post, user) = query_result
    visible_department_ids = get_viewable_departments_ids(db, current_user_id)
    if post.department_id not in visible_department_ids:
        raise HTTPException(status_code=404)

    query_file_result = (
        db.query(models.PostFile).filter(models.PostFile.post_id == post_id).all()
    )
    data = {
        **post.__dict__,
        "files": [],
        "owner_name": '匿名用戶' if post.is_anonymous else user.readable_name if user.readable_name else user.username,
        "owner_id": 'anonymous' if post.is_anonymous else post.owner_id
    }

    for file in query_file_result:
        data["files"].append(file.__dict__["url"])

    return data


def make_post(db: Session, post: Dict, user_id: str, file_array: List[bytes]):
    department = get_department_information(db, post["department_id"])
    db_post = models.Post(
        **post,
        status="APPROVED" if department.is_public else "PENDING",
        owner_id=user_id,
    )
    db.add(db_post)

    db.flush()

    if len(file_array) > 0:
        for file in file_array:
            key = f"{user_id}/{db_post.id}/{str(uuid.uuid4())}"
            r2.put_object(Body=file, Bucket=os.getenv("R2_BUCKET_NAME"), Key=key)
            file_path = f'{os.getenv("R2_FILE_PATH")}/{key}'
            file = {"url": file_path, "post_id": db_post.id}
            db_file = models.PostFile(**file)
            db.add(db_file)

    db.commit()
    db.refresh(db_post)

    if not department.is_public:
        admins = get_department_admins(db=db, department_id=post["department_id"])
        recipients = []
        for admin in admins:
            recipients.append(
                admin.email
                if admin.email is not None
                else f"s{admin.username}@gm.ntpu.edu.tw"
            )
        if len(recipients) > 0:
            send_notification_mail(
                title="考古題審核",
                content=f"有人在您所管理的 {department.name} 社群發布考古題，請儘速審核。",
                recipients=recipients,
                cta={
                    "text": "前往審核",
                    "link": f"{domain}/admin/{department.id}?open_edit_post_dialog=true&edit_post_detail_id={db_post.id}",
                },
            )
    return db_post


def update_post_status(db: Session, post_id: str, status: str):
    post = db.query(models.Post).filter(models.Post.id == post_id).first()
    post.status = status
    db.commit()

    user = db.query(User).filter(User.id == post.owner_id).first()
    department = get_department_information(db=db, department_id=post.department_id)

    if status == "APPROVED":
        send_notification_mail(
            title="考古題審核通過",
            content=f"您在『{department.name}』發布標題為『{post.title}』的考古題已通過管理員的審核。",
            recipients=[
                user.email
                if user.email is not None
                else f"s{user.username}@gm.ntpu.edu.tw"
            ],
            cta={
                "text": "查看考古題",
                "link": f"{domain}/{department.id}/{post.course_id}/{post.id}",
            },
        )
