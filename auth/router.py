import json
from typing import Annotated, Optional

from dotenv import load_dotenv
from fastapi import APIRouter, Depends, Form, HTTPException, Request, status
from fastapi.security import OAuth2PasswordRequestForm
from fastapi_cache.decorator import cache
from jose import ExpiredSignatureError, JWTError
import bcrypt
from pydantic import BaseModel
from sqlalchemy.orm import Session

from departments import dependencies as departments_dependencies
from sql.database import get_db
from users import dependencies as users_dependencies
from utils.token import create_access_token, get_access_token_payload

from . import dependencies

load_dotenv()

router = APIRouter()


class Token(BaseModel):
    access_token: str
    token_type: str


class TokenData(BaseModel):
    username: Optional[str] = None


credentials_exception = HTTPException(
    status_code=status.HTTP_401_UNAUTHORIZED,
    detail="Invalid credentials",
)


async def auth_middleware(request: Request):
    try:
        payload = get_access_token_payload(request)
        username: str = payload.get("sub")
        token_type: str = payload.get("type")
        if username is None or not token_type == "access":
            raise credentials_exception
    except ExpiredSignatureError:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Credentials Expired",
        )
    except JWTError:
        raise credentials_exception


async def admin_middleware(request: Request):
    department_id = request.path_params.get("department_id")
    payload = get_access_token_payload(request)

    if payload is None:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)
    is_super_user: bool = payload.get("isu")
    admin_ids: str = payload.get("adm")

    if admin_ids is None:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)

    admin_ids = json.loads(admin_ids)

    is_department_admin = is_super_user or (
        department_id is not None and department_id in admin_ids
    )

    if not is_department_admin:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)


async def super_user_middleware(request: Request):
    payload = get_access_token_payload(request)
    try:
        if not payload["isu"]:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)
    except KeyError:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN)


def get_password_hash(password):
    return bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")


def verify_password(plain_password, hashed_password):
    return bcrypt.checkpw(plain_password.encode("utf-8"), hashed_password.encode("utf-8"))


@router.post("/exchange")
def login_with_google(
    code: Annotated[str, Form()],
    redirect_uri: Annotated[str, Form()],
    db: Session = Depends(get_db),
):
    user_data_from_google = dependencies.exchange_token_with_google(
        code=code, redirect_uri=redirect_uri
    )
    school_id = str(user_data_from_google["email"])[1:10]
    user = users_dependencies.get_user_by_username(db, school_id)

    if not user:
        user = users_dependencies.create_user(
            db,
            {
                "username": school_id,
                "readable_name": user_data_from_google["name"],
                "school_department": " ",
                "email": user_data_from_google["email"],
            },
        )
    else:
        users_dependencies.update_user(
            db,
            {
                "username": school_id,
                "readable_name": user_data_from_google["name"],
                "school_department": user.school_department,
                "email": user_data_from_google["email"],
            },
        )

    department_admin_ids = users_dependencies.get_user_department_admin_ids(db, user.id)

    access_token = create_access_token(
        data={
            "sub": user.id,
            "type": "access",
            "id": user.id,
            "isu": user.is_super_user,
            "adm": json.dumps(department_admin_ids),
        }
    )
    refresh_token = create_access_token(
        data={
            "sub": user.id,
            "type": "refresh",
            "id": user.id,
            "isu": user.is_super_user,
            "adm": json.dumps(department_admin_ids),
        },
        expires_delta=365,
    )

    return {
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "bearer",
    }


@router.post("/login")
def login(
    form_data: Annotated[OAuth2PasswordRequestForm, Depends()],
    db: Session = Depends(get_db),
):
    user = users_dependencies.get_user_by_username(db, form_data.username)
    try:
        lms_user_info = dependencies.get_lms_user_info(
            username=str(int(form_data.username)), password=form_data.password
        )

        user_dict = {
            "username": form_data.username,
            "readable_name": lms_user_info["readable_name"],
            "school_department": lms_user_info["department"],
            "email": lms_user_info["email"],
        }
        if not user:
            user = users_dependencies.create_user(
                db,
                user_dict,
            )
        else:
            users_dependencies.update_user(
                db,
                user_dict,
            )
    except ValueError:
        if not user or not user.hashed_password:
            raise HTTPException(
                status_code=400, detail="incorrect username or password"
            )
        if not verify_password(form_data.password, user.hashed_password):
            raise HTTPException(
                status_code=400, detail="incorrect username or password"
            )

    department_admin_ids = users_dependencies.get_user_department_admin_ids(db, user.id)

    access_token = create_access_token(
        data={
            "sub": user.id,
            "type": "access",
            "isu": user.is_super_user,
            "id": user.id,
            "adm": json.dumps(department_admin_ids),
        }
    )
    refresh_token = create_access_token(
        data={
            "sub": user.id,
            "type": "refresh",
            "isu": user.is_super_user,
            "id": user.id,
            "adm": json.dumps(department_admin_ids),
        },
        expires_delta=365,
    )

    return {
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "bearer",
    }


@router.get("/verify-token")
@cache(expire=60)
def verify(request: Request, db: Session = Depends(get_db)):
    try:
        payload = get_access_token_payload(request, options={"verify_exp": False})
        user_id = payload.get("id")

        admin_scope = users_dependencies.get_user_department_admin_ids(db, user_id)
        visible_departments = departments_dependencies.get_viewable_departments_ids(
            db, user_id
        )

        permission_data = {
            "admin": admin_scope,
            "visible_departments": visible_departments,
        }
        return permission_data
    except JWTError:
        raise credentials_exception


@router.post("/refresh")
def refresh(request: Request, db: Session = Depends(get_db)):
    try:
        payload = get_access_token_payload(request, token_type="refresh")
        user_id: str = payload.get("id")
        department_admin_ids = users_dependencies.get_user_department_admin_ids(
            db, user_id
        )
        user = users_dependencies.get_user(db, user_id)
        access_token = create_access_token(
            data={
                "sub": user.id,
                "type": "access",
                "isu": user.is_super_user,
                "id": user.id,
                "adm": json.dumps(department_admin_ids),
            },
        )
        refresh_token = create_access_token(
            data={
                "sub": user.id,
                "type": "refresh",
                "isu": user.is_super_user,
                "id": user.id,
                "adm": json.dumps(department_admin_ids),
            },
            expires_delta=365,
        )

        return {
            "access_token": access_token,
            "refresh_token": refresh_token,
            "token_type": "bearer",
        }
    except JWTError:
        raise credentials_exception


@router.post("/create_user", dependencies=[Depends(super_user_middleware)])
def create_user(
    form_data: Annotated[OAuth2PasswordRequestForm, Depends()],
    db: Session = Depends(get_db),
):
    if not form_data.username or not form_data.password:
        raise HTTPException(status_code=400, detail="Invalid input")

    user = users_dependencies.get_user_by_username(db, form_data.username)
    if user:
        raise HTTPException(status_code=400, detail="Same username already exist.")

    hashed_password = get_password_hash(form_data.password)

    user = users_dependencies.create_user(
        db,
        {
            "username": form_data.username,
            "hashed_password": hashed_password,
            "readable_name": form_data.username,
        },
    )

    access_token = create_access_token(data={"sub": user.username, "type": "access", "isu": False, "id": user.id, "adm": "[]"})

    refresh_token = create_access_token(
        data={"sub": user.username, "type": "refresh", "isu": False, "id": user.id, "adm": "[]"},
        expires_delta=365,
    )

    return {
        "access_token": access_token,
        "refresh_token": refresh_token,
        "token_type": "bearer",
    }
