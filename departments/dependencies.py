from typing import Dict

from fastapi import HTTPException
from sqlalchemy.orm import Session

from bulletins.models import Bulletin
from courses.models import Course
from users.models import User, UserDepartment

from . import models


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
    return db.query(models.Department).all()


def get_viewable_departments(db: Session, user_id: str):
    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "APPROVED")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    viewable_departments = db.query(models.Department).filter(sub_query.exists()).all()

    return viewable_departments


def get_departments_status(db: Session, user_id: str):
    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "APPROVED")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    visible_departments = db.query(models.Department).filter(sub_query.exists()).all()

    sub_query = db.query(UserDepartment.department_id).filter(
        (UserDepartment.status == "PENDING")
        & (UserDepartment.user_id == user_id)
        & (models.Department.id == UserDepartment.department_id)
    )

    pending_departments = db.query(models.Department).filter(sub_query.exists()).all()

    return {
        "visible": visible_departments,
        "pending": pending_departments,
    }


def request_view_department(db: Session, request: Dict):
    if (
        db.query(models.Department)
        .filter(models.Department.id == request["department_id"])
        .first()
        is None
    ):
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
    return db_request


def approve_request_view_department(db: Session, request_id: str):
    request = db.query(UserDepartment).filter(UserDepartment.id == request_id).first()
    if request is None:
        raise HTTPException(status_code=404)

    request.status = "APPROVED"
    db.commit()


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
            UserDepartment.id, User.id, User.email, User.username, User.readable_name
        )
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.status == "PENDING")
        )
        .join(User, User.id == UserDepartment.user_id)
        .all()
    )

    result = []

    for request_id, user_id, email, username, readable_name in requests:
        result.append(
            {
                "status": "PENDING",
                "user_id": user_id,
                "email": email,
                "username": username,
                "readable_name": readable_name,
                "id": request_id,
            }
        )

    return result


def get_department_members(db: Session, department_id: str):
    members = (
        db.query(
            UserDepartment.is_department_admin,
            User.id,
            User.email,
            User.username,
            User.readable_name,
        )
        .filter(
            (UserDepartment.department_id == department_id)
            & (UserDepartment.status == "APPROVED")
        )
        .join(User, User.id == UserDepartment.user_id)
        .all()
    )

    result = []

    for is_department_admin, user_id, email, username, readable_name in members:
        result.append(
            {
                "id": user_id,
                "user_id": user_id,
                "email": email,
                "username": username,
                "readable_name": readable_name,
                "is_department_admin": is_department_admin,
            }
        )

    return result
