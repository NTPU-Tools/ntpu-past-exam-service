import json
import os
import pickle
from contextlib import asynccontextmanager
from typing import Any

from dotenv import load_dotenv
from fastapi import Depends, FastAPI, Request, Response
from fastapi.exception_handlers import http_exception_handler
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.security import OAuth2PasswordBearer
from fastapi_cache import Coder, FastAPICache
from fastapi_cache.backends.redis import RedisBackend
from redis import asyncio as aioredis
from starlette.exceptions import HTTPException

from auth.router import auth_middleware
from auth.router import router as auth_router
from bulletins.router import router as bulletins_router
from courses.router import router as courses_router
from departments.router import router as departments_router
from posts.router import router as posts_router
from thread.router import router as thread_router
from sql import models
from sql.database import engine
from users.router import router as users_router
from utils.exception_handlers import (
    request_validation_exception_handler,
    unhandled_exception_handler,
)
from utils.log import log_request_middleware

oauth2_scheme = OAuth2PasswordBearer(tokenUrl="login")

models.Base.metadata.create_all(bind=engine)

load_dotenv()


# pylint: disable-next=keyword-arg-before-vararg
def request_key_builder(
    # pylint: disable-next=unused-argument
    func,
    namespace: str = "",
    request: Request = None,
    # pylint: disable-next=unused-argument
    response: Response = None,
    # pylint: disable-next=unused-argument
    *args,
    # pylint: disable-next=unused-argument
    **kwargs,
):
    return ":".join(
        [
            namespace,
            request.method.lower(),
            request.url.path,
            request.headers.get("authorization"),
            repr(sorted(request.query_params.items())),
        ]
    )


class ORMJsonCoder(Coder):
    @classmethod
    def encode(cls, value: Any) -> bytes:
        return json.dumps(value, default=str).encode()

    @classmethod
    def decode(cls, value: bytes) -> Any:
        return json.loads(value)


@asynccontextmanager
async def lifespan(app: FastAPI):
    redis = aioredis.Redis(
        host=os.getenv("REDIS_HOST"),
        password=os.getenv("REDIS_PASSWORD"),
        port=int(os.getenv("REDIS_PORT")),
    )
    FastAPICache.init(
        RedisBackend(redis),
        prefix="fastapi-cache",
        key_builder=request_key_builder,
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

origins = [
    "https://past-exam.zeabur.app",
    "https://past-exam.ntpu.cc",
    "https://past-exam.ntpu.xyz",
    "http://localhost",
    "http://localhost:3000",
    "http://localhost:8080",
]

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
    dependencies=[Depends(auth_middleware), Depends(oauth2_scheme)],
)
app.include_router(auth_router, tags=["Auth"])


@app.get("/ping", tags=["Health Check"])
def heartbeat():
    return "pong"


@app.get("/system-version", tags=["Health Check"])
def get_system_version():
    return {
        "GIT_SHA": os.getenv("COMMIT_SHA")
    }