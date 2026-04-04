import os
from datetime import datetime, timedelta, timezone
from typing import Optional

from dotenv import load_dotenv
from fastapi import HTTPException, Request
from jose import JWTError, jwt

load_dotenv()


SECRET_KEY = os.getenv("HASH_KEY")
if not SECRET_KEY:
    raise RuntimeError("HASH_KEY environment variable is not set or empty")
ALGORITHM = "HS256"


def get_access_token_payload(request: Request, options=None, token_type: str = "access"):
    auth_header = request.headers.get("authorization") or ""
    parts = auth_header.split(" ")
    if len(parts) != 2 or parts[0] != "Bearer":
        raise HTTPException(status_code=401, detail="Invalid or missing authorization header")
    try:
        payload = jwt.decode(parts[1], SECRET_KEY, algorithms=[ALGORITHM], options=options)
    except JWTError:
        raise HTTPException(status_code=401, detail="Invalid or expired token")
    if token_type is not None:
        payload_type = payload.get("type")
        # Backward compat: tokens issued before type field was added had no "type";
        # treat missing type as "access" during the 1-day transition window.
        if not (token_type == "access" and payload_type is None) and payload_type != token_type:
            raise HTTPException(status_code=401, detail="Invalid token type")
    return payload


def create_access_token(data: dict, expires_delta: Optional[int] = 1):
    to_encode = data.copy()
    expire = datetime.now(timezone.utc) + timedelta(days=expires_delta)

    to_encode.update({"exp": expire})
    encoded_jwt = jwt.encode(to_encode, SECRET_KEY, algorithm=ALGORITHM)
    return encoded_jwt
