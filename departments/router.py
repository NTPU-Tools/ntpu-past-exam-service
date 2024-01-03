from typing import Annotated

from fastapi import APIRouter, Depends, Form, Request
from fastapi_cache.decorator import cache
from sqlalchemy.orm import Session

from auth.router import admin_middleware
from sql.database import get_db
from utils.token import get_access_token_payload

from . import dependencies

router = APIRouter(prefix="/departments")


@router.get("")
def read_all_departments(db: Session = Depends(get_db)):
    items = dependencies.get_departments(db)
    return items


@router.get("/{department_id}/courses")
@cache(expire=60)
def get_department_courses(department_id: str, db: Session = Depends(get_db)):
    return dependencies.get_department_courses(db, department_id=department_id)


@router.get("/{department_id}/bulletins")
@cache(expire=60)
def get_department_bulletins(department_id: str, db: Session = Depends(get_db)):
    return dependencies.get_department_bulletins(db, department_id=department_id)


@router.get("/{department_id}/visible-check")
def check_user_can_view(
    department_id: str, request: Request, db: Session = Depends(get_db)
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    return {
        "can_view": dependencies.check_can_view(
            db, user_id=user_id, department_id=department_id
        )
    }


@router.get("/{department_id}/admin-check")
def check_user_is_department_admin(
    department_id: str, request: Request, db: Session = Depends(get_db)
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    return {
        "is_admin": dependencies.check_is_department_admin(
            db, user_id=user_id, department_id=department_id
        )
    }


@router.get("/status")
@cache(expire=60)
def read_user_departments_status(request: Request, db: Session = Depends(get_db)):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    return dependencies.get_departments_status(db, user_id)


@router.get("/visible")
@cache(expire=60)
def read_user_viewable_departments(request: Request, db: Session = Depends(get_db)):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    items = dependencies.get_viewable_departments(db, user_id)
    return items


@router.post("/{department_id}/join-request/send")
def send_join_request(
    request: Request, department_id: str, db: Session = Depends(get_db)
):
    payload = get_access_token_payload(request)
    user_id = payload.get("id")
    join_request = {"department_id": department_id, "user_id": user_id}
    data = dependencies.request_view_department(db, join_request)

    return data.__dict__


@router.get("/{department_id}/join-request", dependencies=[Depends(admin_middleware)])
def get_join_request(department_id: str, db: Session = Depends(get_db)):
    data = dependencies.get_join_requests(db, department_id)
    return data


@router.get("/{department_id}")
def get_department_information(department_id: str, db: Session = Depends(get_db)):
    data = dependencies.get_department_information(db, department_id)
    return data


@router.get("/{department_id}/members", dependencies=[Depends(admin_middleware)])
def get_department_members(department_id: str, db: Session = Depends(get_db)):
    data = dependencies.get_department_members(db, department_id)
    return data


@router.put("/{department_id}/admin", dependencies=[Depends(admin_middleware)])
def update_member_admin(
    user_id: Annotated[str, Form()],
    is_admin: Annotated[bool, Form()],
    department_id: str,
    db: Session = Depends(get_db),
):
    dependencies.update_member_admin(
        db, department_id=department_id, user_id=user_id, is_admin=is_admin
    )
    return {"status": "success"}


@router.put(
    "/join-request/{department_id}/approve/{request_id}",
    dependencies=[Depends(admin_middleware)],
)
async def approve_join_request(
    request_id: str,
    db: Session = Depends(get_db),
):
    dependencies.approve_request_view_department(db, request_id)

    return {"status": "success"}
