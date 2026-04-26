from fastapi import HTTPException
from sqlalchemy.orm import Session

from departments.models import Department
from . import models, schemas


def update_user_status(db: Session, user_id: str, status: bool):
    (
        db.query(models.User)
        .filter(models.User.id == user_id)
        .update({"is_active": status})
    )
    db.commit()


def update_user_admin(db: Session, user_id: str, is_admin: bool):
    (
        db.query(models.User)
        .filter(models.User.id == user_id)
        .update({"is_admin": is_admin})
    )
    db.commit()


def get_user_department_admin_ids(db: Session, user_id: str):
    user = db.query(models.User).filter(models.User.id == user_id).first()
    if not user:
        raise HTTPException(status_code=401, detail="User not found")
    is_admin_departments = []
    if user.is_super_user:
        for department in db.query(Department).all():
            is_admin_departments.append(department.id)

    else:
        for record in (
            db.query(models.UserDepartment)
            .filter(
                (models.UserDepartment.is_department_admin == True)
                & (models.UserDepartment.user_id == user_id)
            )
            .all()
        ):
            is_admin_departments.append(record.department_id)

    return is_admin_departments


def get_user_department_admin(db: Session, user_id: str):
    user = db.query(models.User).filter(models.User.id == user_id).first()
    if not user:
        raise HTTPException(status_code=401, detail="User not found")
    if user.is_super_user:
        return db.query(Department).all()
    sub_query = db.query(models.UserDepartment.department_id).filter(
        (models.UserDepartment.is_department_admin == True)
        & (models.UserDepartment.user_id == user_id)
        & (models.UserDepartment.department_id == Department.id)
    )

    items = db.query(Department).filter(sub_query.exists()).all()
    return items


def get_user(db: Session, user_id: str):
    return db.query(models.User).filter(models.User.id == user_id).first()


def get_user_by_email(db: Session, email: str):
    return db.query(models.User).filter(models.User.email == email).first()


def get_user_by_username(db: Session, username: str):
    return db.query(models.User).filter(models.User.username == username).first()


def get_users(db: Session, is_active: bool, skip: int = 0, limit: int = 100):
    query_filter = []

    if is_active is not None:
        if is_active:
            query_filter.append(models.User.is_active == True)
        else:
            query_filter.append(
                (models.User.is_active == False) | (models.User.is_active == None)
            )

    return db.query(models.User).filter(*query_filter).offset(skip).limit(limit).all()


def get_user_preference(db: Session, user_id: str):
    return (
        db.query(models.UserPreference)
        .filter(models.UserPreference.user_id == user_id)
        .first()
    )


def update_user_preference(db: Session, user_id: str, show_empty_courses: bool):
    pref = (
        db.query(models.UserPreference)
        .filter(models.UserPreference.user_id == user_id)
        .first()
    )
    if pref:
        pref.show_empty_courses = show_empty_courses
    else:
        pref = models.UserPreference(
            user_id=user_id, show_empty_courses=show_empty_courses
        )
        db.add(pref)
    db.commit()
    db.refresh(pref)
    return pref


def create_user(db: Session, user: schemas.UserCreate):
    user_kwargs = {
        "username": user["username"],
        "readable_name": user["readable_name"],
        "school_department": user["school_department"],
        "email": user["email"],
    }
    if user.get("hashed_password"):
        user_kwargs["hashed_password"] = user["hashed_password"]
    db_user = models.User(**user_kwargs)
    db.add(db_user)
    db.flush()
    db_pref = models.UserPreference(user_id=db_user.id, show_empty_courses=True)
    db.add(db_pref)
    db.commit()
    db.refresh(db_user)
    return db_user


def update_user(db: Session, user: schemas.UserCreate):
    update_data = {
        "username": user.get('username'),
        "readable_name": user.get('readable_name'),
        "school_department": user.get('school_department'),
        "email": user.get('email'),
        "note": user.get('note'),
    }
    if user.get('hashed_password'):
        update_data["hashed_password"] = user['hashed_password']
    db.query(models.User).filter(models.User.username == user["username"]).update(update_data)
    db.commit()
