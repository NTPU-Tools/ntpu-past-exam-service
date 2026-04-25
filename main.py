import hashlib
import inspect
import json
import os
from contextlib import asynccontextmanager
from datetime import datetime
from typing import Any, Callable, Dict, Optional, Tuple

from alembic.config import Config
from dotenv import load_dotenv
from fastapi import Depends, FastAPI, Request, Response
from fastapi.exception_handlers import http_exception_handler
from fastapi.encoders import jsonable_encoder
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.security import OAuth2PasswordBearer
from fastapi_cache import Coder, FastAPICache
from fastapi_cache.backends.redis import RedisBackend
from redis import asyncio as aioredis
from sqlalchemy.orm import Session
from starlette.exceptions import HTTPException

from alembic import command
from auth.router import auth_middleware
from auth.router import router as auth_router
from bulletins.router import router as bulletins_router
from courses.router import router as courses_router
from departments.router import router as departments_router
from posts.router import router as posts_router
from thread.router import router as thread_router
from users.router import router as users_router
from utils.exception_handlers import (
    request_validation_exception_handler,
    unhandled_exception_handler,
)
from utils.log import log_request_middleware

oauth2_scheme = OAuth2PasswordBearer(tokenUrl="login")

load_dotenv()


def _is_request_annotation(annotation: Any) -> bool:
    """Check whether an annotation is ``Request`` or ``Optional[Request]`` /
    ``Request | None``.
    """
    import typing

    if annotation is Request:
        return True

    # Handle Optional[T] / Union[T, None]
    origin = getattr(annotation, "__origin__", None)
    if origin is not None:
        # typing.Optional or typing.Union
        args = getattr(annotation, "__args__", ())
        return any(arg is Request for arg in args)

    return False


def custom_key_builder(
    func: Callable[..., Any],
    namespace: str = "",
    *,
    request: Optional[Request] = None,
    response: Optional[Response] = None,
    args: Tuple[Any, ...] = (),
    kwargs: Optional[Dict[str, Any]] = None,
) -> str:
    """Build cache key that excludes SQLAlchemy Session and avoids
    over-isolating public data by user auth.

    - If the original endpoint signature explicitly declares a ``request: Request``
      parameter, the authorization header hash is included so that user-specific
      responses (e.g. ``/users/me``) are not leaked across users.
    - For public endpoints (no ``request`` parameter), the auth header is ignored
      so that all users share the same cache entry.
    """
    kwargs = kwargs or {}

    # 1. Filter out SQLAlchemy Session objects — FastAPI creates a new Session
    #    per request, so including it would make the cache key unique every time.
    filtered_kwargs = {
        k: v for k, v in kwargs.items() if not isinstance(v, Session)
    }

    # 2. Base key derived from function identity + arguments (hashed to keep
    #    key length bounded while keeping namespace and func name readable).
    args_hash = hashlib.md5(
        f"{func.__module__}:{func.__name__}:{args}:{filtered_kwargs}".encode()
    ).hexdigest()
    cache_key = f"{func.__name__}:{args_hash}"

    # 3. Detect whether the *original* endpoint accepts ``request: Request``.
    #    The cache decorator may inject its own request parameter, so we inspect
    #    the wrapped function instead of relying on the ``request`` argument.
    sig = inspect.signature(func)
    has_explicit_request = any(
        _is_request_annotation(p.annotation) for p in sig.parameters.values()
    )

    if has_explicit_request and request is not None:
        auth = request.headers.get("authorization", "")
        auth_hash = hashlib.md5(auth.encode()).hexdigest()
        return f"{namespace}:{auth_hash}:{cache_key}"

    return f"{namespace}:{cache_key}"


def _json_default(obj: Any) -> Any:
    if isinstance(obj, datetime):
        return obj.isoformat()
    return str(obj)


class ORMJsonCoder(Coder):
    @classmethod
    def encode(cls, value: Any) -> bytes:
        return json.dumps(jsonable_encoder(value), default=_json_default).encode()

    @classmethod
    def decode(cls, value: bytes) -> Any:
        try:
            return json.loads(value)
        except (json.JSONDecodeError, UnicodeDecodeError) as exc:
            raise ValueError("Failed to decode cached value") from exc


@asynccontextmanager
async def lifespan(app: FastAPI):
    alembic_cfg = Config("alembic.ini")
    command.upgrade(alembic_cfg, "head")

    redis = aioredis.Redis(
        host=os.getenv("REDIS_HOST"),
        password=os.getenv("REDIS_PASSWORD"),
        port=int(os.getenv("REDIS_PORT")),
    )
    FastAPICache.init(
        RedisBackend(redis),
        prefix="fastapi-cache:v2",
        key_builder=custom_key_builder,
        coder=ORMJsonCoder,
    )
    yield


app = FastAPI(
    docs_url=None,
    redoc_url=None,
    lifespan=lifespan,
)

app.middleware("http")(log_request_middleware)
app.add_exception_handler(RequestValidationError, request_validation_exception_handler)
app.add_exception_handler(HTTPException, http_exception_handler)
app.add_exception_handler(Exception, unhandled_exception_handler)


@app.middleware("http")
async def cache_control_middleware(request: Request, call_next):
    """Prevent browsers from caching dynamic API responses.

    This ensures that frontend SWR revalidation always fetches fresh data
    from the server instead of serving stale responses from the browser's
    HTTP cache. Server-side Redis caching (fastapi-cache2) is unaffected.
    """
    response = await call_next(request)
    if request.method == "GET" and 200 <= response.status_code < 300:
        response.headers["Cache-Control"] = "no-store"
    return response

origins = [
    "https://past-exam.zeabur.app",
    "https://past-exam.ntpu.cc",
    "https://past-exam.ntpu.xyz",
    "http://localhost",
    "http://localhost:5173",
    "http://localhost:8080",
]

origin_from_env = os.getenv("ORIGIN")
if origin_from_env and origin_from_env not in origins:
    origins.append(origin_from_env)

app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(
    departments_router,
    tags=["Departments"],
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)
app.include_router(
    users_router,
    tags=["Users"],
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)
app.include_router(
    courses_router,
    tags=["Courses"],
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)
app.include_router(
    posts_router,
    tags=["Posts"],
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)

app.include_router(
    bulletins_router,
    tags=["Bulletins"],
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)
app.include_router(
    thread_router,
    tags=["Threads"],
)
app.include_router(auth_router, tags=["Auth"])


@app.get("/ping", tags=["Health Check"])
def heartbeat():
    return "pong"


@app.get("/system-version", tags=["Health Check"])
def get_system_version():
    return {"GIT_SHA": os.getenv("COMMIT_SHA")}
