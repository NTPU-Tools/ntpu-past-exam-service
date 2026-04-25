from typing import Annotated

from fastapi import APIRouter, Depends, Form, HTTPException
from fastapi.encoders import jsonable_encoder
from fastapi_cache.decorator import cache
from sqlalchemy.orm import Session
from starlette.concurrency import run_in_threadpool

from auth.router import admin_middleware
from sql.database import get_db
from utils.cache import clear_namespace

from . import dependencies, schemas

router = APIRouter(prefix="/courses")


@router.get("", response_model=list[schemas.Course])
@cache(expire=60, namespace="courses")
def read_courses(skip: int = 0, limit: int = 100, db: Session = Depends(get_db)):
    items = dependencies.get_courses(db, skip=skip, limit=limit)
    return items


@router.get("/{course_id}")
@cache(expire=30, namespace="course-detail")
def get_course(course_id: str, db: Session = Depends(get_db)):
    data = dependencies.get_course(db, course_id)

    if data["course"] is None:
        raise HTTPException(status_code=404, detail="Course not found")

    formatted_data = {
        "course": jsonable_encoder(data["course"]),
        "posts": [],
    }
    for post in data["posts"]:
        post_dict = jsonable_encoder(post)
        if post_dict.get("status") == "APPROVED":
            post_dict.pop("status", None)
            post_dict.pop("owner_id", None)
            formatted_data["posts"].append(post_dict)
    return formatted_data


@router.post("/{department_id}", dependencies=[Depends(admin_middleware)])
async def create_course(
    name: Annotated[str, Form()],
    category: Annotated[str, Form()],
    department_id: str,
    db: Session = Depends(get_db),
):
    course = {"name": name, "category": category, "department_id": department_id}
    course = await run_in_threadpool(dependencies.make_course, db, course)
    await clear_namespace("courses")
    await clear_namespace("dept-courses")
    return jsonable_encoder(course)
