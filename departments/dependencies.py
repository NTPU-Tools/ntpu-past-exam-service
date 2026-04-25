import os
from typing import Dict

from dotenv import load_dotenv
from fastapi import HTTPException
from sqlalchemy.orm import Session

from bulletins.models import Bulletin
from courses.models import Course
from users.models import User, UserDepartment
from utils.send_mail import send_notification_mail

from . import models

load_dotenv()
domain = os.getenv("ORIGIN")


def get_department_bulletins(db: Session, department_id: str):
    return db.query(Bulletin).filter(Bulletin.department_id == department_id).all()


def get_department_courses(db: Session, department_id: str):
    return db.query(Course).filter(Course.department_id == department_id).all()


def check_can_view(db: Session, user_id: str, department_id: str):
    return (
        db.query(UserDepartment)
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.user_id == user_id)
            & (UserDepartment.status == "APPROVED")
        )
        .first()
    ) is not None


def check_is_department_admin(db: Session, user_id: str, department_id: str):
    return (
        db.query(UserDepartment)
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.user_id == user_id)
            & (UserDepartment.is_department_admin)
        )
        .first()
    ) is not None


def get_departments(db: Session):
    return db.query(models.Department).order_by(models.Department.sort_order.asc()).all()


def get_viewable_departments(db: Session, user_id: str):
    user = db.query(User).filter(User.id == user_id).first()
    if not user:
        raise HTTPException(status_code=401, detail="User not found")
    if user.is_super_user:
        return db.query(models.Department).order_by(models.Department.sort_order.asc()).all()
    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "APPROVED")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    visible_departments = (
        db.query(models.Department)
        .filter((sub_query.exists()) | (models.Department.is_public == True))
        .order_by(models.Department.sort_order.asc())
        .all()
    )

    return visible_departments


def get_viewable_departments_ids(db: Session, user_id: str):
    user = db.query(User).filter(User.id == user_id).first()
    viewable_departments = []
    if user.is_super_user:
        for department in db.query(models.Department).order_by(models.Department.sort_order.asc()).all():
            viewable_departments.append(department.id)

    else:
        sub_query = db.query(UserDepartment.department_id).filter(
            (UserDepartment.status == "APPROVED")
            & (UserDepartment.user_id == user_id)
            & (models.Department.id == UserDepartment.department_id)
        )

        visible_departments = (
            db.query(models.Department)
            .filter((sub_query.exists()) | (models.Department.is_public == True))
            .order_by(models.Department.sort_order.asc())
            .all()
        )

        for record in visible_departments:
            viewable_departments.append(record.id)

    return viewable_departments


def get_departments_status(db: Session, user_id: str):
    user = db.query(User).filter(User.id == user_id).first()
    if user.is_super_user:
        return {
            "visible": db.query(models.Department).order_by(models.Department.sort_order.asc()).all(),
            "pending": [],
        }
    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "APPROVED")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    visible_departments = (
        db.query(models.Department)
        .filter((sub_query.exists()) | (models.Department.is_public == True))
        .order_by(models.Department.sort_order.asc())
        .all()
    )

    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "PENDING")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    pending_departments = (
        db.query(models.Department)
        .filter(sub_query.exists())
        .order_by(models.Department.sort_order.asc())
        .all()
    )

    return {
        "visible": visible_departments,
        "pending": pending_departments,
    }


def request_view_department(db: Session, request: Dict):
    department = (
        db.query(models.Department)
        .filter(models.Department.id == request["department_id"])
        .first()
    )

    if department is None:
        raise HTTPException(status_code=404)

    past_request = (
        db.query(UserDepartment)
        .filter(
            (UserDepartment.department_id == request["department_id"])
            & (UserDepartment.user_id == request["user_id"])
        )
        .first()
    )
    if past_request is not None:
        if past_request.status == "APPROVED":
            raise HTTPException(status_code=400, detail="request has been approved")
        raise HTTPException(status_code=400, detail="duplicate request")

    db_request = UserDepartment(**request)
    db.add(db_request)
    db.commit()
    db.refresh(db_request)

    admins = get_department_admins(db=db, department_id=department.id)

    recipients = []

    for admin in admins:
        recipients.append(
            admin.email
            if admin.email is not None
            else f"s{admin.username}@gm.ntpu.edu.tw"
        )

    if len(recipients) > 0:
        send_notification_mail(
            title="請求審核",
            content=f"有人申請瀏覽您所管理的 {department.name} 社群，請儘速審核。",
            recipients=recipients,
            cta={
                "text": "前往審核",
                "link": f"{domain}/admin/{department.id}?open_edit_member_dialog=true",
            },
        )

    return db_request


def approve_request_view_department(db: Session, request_id: str):
    request = db.query(UserDepartment).filter(UserDepartment.id == request_id).first()
    if request is None:
        raise HTTPException(status_code=404)

    request.status = "APPROVED"
    db.commit()

    department = (
        db.query(models.Department)
        .filter(models.Department.id == request.department_id)
        .first()
    )

    user = db.query(User).filter(User.id == request.user_id).first()

    send_notification_mail(
        title="審核通過",
        content=f"您申請加入 {department.name} 的審核已通過。",
        recipients=[
            user.email if user.email is not None else f"s{user.username}@gm.ntpu.edu.tw"
        ],
    )


def update_member_admin(db: Session, department_id: str, user_id: str, is_admin: bool):
    request = (
        db.query(UserDepartment)
        .filter(
            (UserDepartment.user_id == user_id)
            & (UserDepartment.department_id == department_id)
        )
        .first()
    )
    if request is None:
        raise HTTPException(status_code=404)

    request.is_department_admin = is_admin
    db.commit()

    department = (
        db.query(models.Department)
        .filter(models.Department.id == request.department_id)
        .first()
    )

    user = db.query(User).filter(User.id == request.user_id).first()

    if is_admin:
        send_notification_mail(
            title="權限變更",
            content=f"您在 {department.name} 的權限已被提升至『管理員』。",
            recipients=[
                user.email
                if user.email is not None
                else f"s{user.username}@gm.ntpu.edu.tw"
            ],
        )
    else:
        send_notification_mail(
            title="權限變更",
            content=f"您在 {department.name} 的權限已被變更至『一般用戶』。",
            recipients=[
                user.email
                if user.email is not None
                else f"s{user.username}@gm.ntpu.edu.tw"
            ],
        )


def get_department_information(db: Session, department_id: str):
    item = (
        db.query(models.Department)
        .filter(models.Department.id == department_id)
        .first()
    )
    if item is None:
        raise HTTPException(status_code=404)

    return item


def get_join_requests(db: Session, department_id: str):
    requests = (
        db.query(
            UserDepartment.id,
            User.id,
            User.email,
            User.username,
            User.readable_name,
            User.school_department,
            User.note,
        )
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.status == "PENDING")
        )
        .join(User, User.id == UserDepartment.user_id)
        .all()
    )

    result = []

    for (
        request_id,
        user_id,
        email,
        username,
        readable_name,
        school_department,
        note,
    ) in requests:
        result.append(
            {
                "status": "PENDING",
                "user_id": user_id,
                "email": email,
                "username": username,
                "readable_name": readable_name,
                "school_department": school_department,
                "note": note,
                "id": request_id,
            }
        )

    return result


def get_department_admins(db: Session, department_id: str):
    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.is_department_admin == True)
        & (UserDepartment.department_id == department_id)
        & (UserDepartment.user_id == User.id)
    )

    admins = db.query(User).filter(sub_query.exists()).all()
    return admins


def get_department_members(db: Session, department_id: str):
    members = (
        db.query(
            UserDepartment.is_department_admin,
            User.id,
            User.email,
            User.username,
            User.readable_name,
            User.school_department,
        )
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.status == "APPROVED")
        )
        .join(User, User.id == UserDepartment.user_id)
        .all()
    )

    result = []

    for (
        is_department_admin,
        user_id,
        email,
        username,
        readable_name,
        school_department,
    ) in members:
        result.append(
            {
                "id": user_id,
                "user_id": user_id,
                "email": email,
                "username": username,
                "readable_name": readable_name,
                "school_department": school_department,
                "is_department_admin": is_department_admin,
            }
        )

    return result
