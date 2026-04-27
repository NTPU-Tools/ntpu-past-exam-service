from typing import Annotated

from fastapi import APIRouter, Body, Depends, Form, HTTPException, Request
from fastapi.encoders import jsonable_encoder
from fastapi_cache.decorator import cache
from sqlalchemy.orm import Session
from starlette.concurrency import run_in_threadpool

from auth.router import admin_middleware
from sql.database import get_db
from utils.cache import clear_namespace
from utils.token import get_access_token_payload

from . import dependencies, schemas

router = APIRouter(prefix="/users")


@router.get("")
def read_users(
    is_active: bool = None,
    skip: int = 0,
    limit: int = 100,
    db: Session = Depends(get_db),
):
    users = dependencies.get_users(db, skip=skip, limit=limit, is_active=is_active)
    data = []
    for user in users:
        u = jsonable_encoder(user)
        u.pop("hashed_password", None)
        data.append(u)
    return data


@router.get("/{user_id}")
@cache(expire=30, namespace="user-detail")
def read_user(request: Request, user_id: str, db: Session = Depends(get_db)):
    is_me = user_id == "me"
    if is_me:
        payload = get_access_token_payload(request)
        user_id: str = payload.get("id")
    db_user = dependencies.get_user(db, user_id=user_id)
    if db_user is None:
        raise HTTPException(status_code=404, detail="User not found")
    data = jsonable_encoder(db_user)
    data.pop("hashed_password", None)
    if is_me:
        pref = dependencies.get_user_preference(db, user_id)
        data["show_empty_courses"] = pref.show_empty_courses if pref else True
    return data


@router.get("/me/departments-admin")
@cache(expire=30, namespace="user-admin")
def read_user_admin_scopes(request: Request, db: Session = Depends(get_db)):
    payload = get_access_token_payload(request)
    user_id: str = payload.get("id")
    scopes = dependencies.get_user_department_admin(db, user_id=user_id)

    return scopes


@router.put("/update/me")
async def update_user_info(
    request: Request,
    major: Annotated[str, Form()],
    note: Annotated[str, Form()] = "",
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id: str = payload.get("id")
    user = await run_in_threadpool(dependencies.get_user, db, user_id)
    if user is None:
        raise HTTPException(status_code=404, detail="User not found")
    await run_in_threadpool(
        dependencies.update_user,
        db,
        {
            "username": user.username,
            "readable_name": user.readable_name,
            "school_department": major,
            "email": user.email,
            "note": note,
        },
    )
    await clear_namespace("user-detail")
    await clear_namespace("verify-token")
    updated_user = await run_in_threadpool(dependencies.get_user, db, user_id)
    data = jsonable_encoder(updated_user)
    data.pop("hashed_password", None)
    pref = dependencies.get_user_preference(db, user_id)
    data["show_empty_courses"] = pref.show_empty_courses if pref else True
    return data


@router.put("/me/preferences")
async def update_user_preferences(
    request: Request,
    preferences: schemas.UserPreferencesUpdate,
    db: Session = Depends(get_db),
):
    payload = get_access_token_payload(request)
    user_id: str = payload.get("id")
    pref = await run_in_threadpool(
        dependencies.update_user_preference,
        db,
        user_id,
        preferences.show_empty_courses,
    )
    await clear_namespace("user-detail")
    await clear_namespace("verify-token")
    return {"show_empty_courses": pref.show_empty_courses if pref else True}


@router.put("/status/{department_id}/{user_id}", dependencies=[Depends(admin_middleware)])
async def update_user_active_status(
    is_active: Annotated[bool, Form()], user_id: str, db: Session = Depends(get_db)
):
    await run_in_threadpool(dependencies.update_user_status, db, user_id=user_id, status=is_active)
    await clear_namespace("user-detail")
    await clear_namespace("verify-token")
    return {"status": "success"}


@router.put("/admin/{department_id}/{user_id}", dependencies=[Depends(admin_middleware)])
async def update_user_admin_status(
    is_admin: Annotated[bool, Form()], user_id: str, db: Session = Depends(get_db)
):
    await run_in_threadpool(dependencies.update_user_admin, db, user_id=user_id, is_admin=is_admin)
    await clear_namespace("user-detail")
    await clear_namespace("user-admin")
    await clear_namespace("verify-token")
    return {"status": "success"}
