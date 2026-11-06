# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

NTPU Past Exam Service — a FastAPI REST API for managing past exam resources at National Taipei University. Handles auth, departments, courses, exam posts with file uploads, and bulletins.

## Commands

```bash
poetry install                                # Install dependencies
uvicorn main:app --reload                     # Start dev server
alembic upgrade head                          # Run all migrations
alembic revision --autogenerate -m "message"  # Generate new migration
```

Uses **Poetry** for dependency management. `requirements.txt` is an export (`scripts/export_requirements.sh`).

### Code Quality

```bash
black .           # Format code
isort .           # Sort imports
pylint **/*.py    # Lint
pre-commit run    # Run all pre-commit hooks
```

Pre-commit hooks configured in `.pre-commit-config.yaml`.

## Tech Stack

- **FastAPI** 0.104.1 (async)
- **SQLAlchemy 2.0** ORM + **PostgreSQL 17** + **Alembic** migrations
- **Redis** caching plus revocable refresh-session state
- **Cloudflare R2** (S3-compatible via boto3) for file storage
- **JWT** auth (HS256, python-jose)
- **Logtail** for structured logging (Better Stack)

## Architecture

Feature-based module organization. Each module contains:
- `router.py` — FastAPI endpoint definitions
- `dependencies.py` — Business logic and database operations
- `models.py` — SQLAlchemy ORM models
- `schemas.py` — Pydantic request/response schemas (where needed)

### Modules

- **`auth/`** — Google OAuth code exchange, NTPU LMS scraping login (BeautifulSoup), JWT token create/refresh, user creation (super user only)
- **`users/`** — User CRUD, department membership status management
- **`departments/`** — Department CRUD, join request flow (send → approve), member/admin management
- **`courses/`** — Course CRUD scoped to departments
- **`posts/`** — Exam post CRUD with multipart file upload to R2, status approval workflow
- **`bulletins/`** — Department announcements
- **`sql/database.py`** — SQLAlchemy engine, session factory, `BaseColumn` mixin (UUID pk + `create_time`/`updated_time`)
- **`sql/models.py`** — Aggregated model imports for Alembic
- **`static_file/r2.py`** — Cloudflare R2 (S3) client initialization
- **`utils/`** — `token.py` (JWT create/verify), `log.py` (Logtail request logging middleware), `exception_handlers.py` (global error handling), `send_mail.py` (AWS SES email)

### Authorization Model

Three middleware tiers applied as FastAPI dependencies:

1. **`auth_middleware`** — Validates JWT access token presence and signature
2. **`admin_middleware`** — Checks `is_department_admin` for the `{department_id}` path param
3. **`super_user_middleware`** — Requires `isu` (is_super_user) flag in JWT

JWT payload structure:
```json
{
  "sub": "user_id",
  "type": "access|refresh",
  "id": "user_id",
  "isu": true,
  "adm": "[\"dept_id_1\", \"dept_id_2\"]"
}
```

Token expiry: access = 1 day, refresh = 365 days.

### Key Business Logic

- **Post approval**: Auto-approved if department `is_public`, otherwise `PENDING` until admin approves
- **Department access**: Public departments visible to all; private require approved `UserDepartment` membership
- **Anonymous posts**: `is_anonymous` flag replaces owner_id with "anonymous" / "匿名用戶" in responses
- **LMS login**: Scrapes NTPU LMS with BeautifulSoup to authenticate students; falls back to bcrypt password check
- **Google OAuth**: Validates `gm.ntpu.edu.tw` email domain, extracts school_id from email chars 1-10

### Caching

Redis-backed with key format `namespace:method:path:authorization:query_params`. Cache durations:
- 30s: user details, admin departments, course with posts
- 60s: department courses/bulletins, department status/visible, all courses, post details, bulletins, verify-token

### Database Models

All models inherit `BaseColumn` (UUID primary key, auto timestamps). Key models:
- `User` — username, email, hashed_password, is_super_user
- `UserDepartment` — join table with status (PENDING/APPROVED) and is_department_admin
- `Department` — name, key, is_public
- `Course` — name, category, department_id
- `Post` — title, content, owner_id, course_id, department_id, status, is_anonymous
- `PostFile` — url, post_id
- `Bulletin` — title, content, department_id

## Environment Variables

```
DATABASE_HOST, DATABASE_USERNAME, DATABASE_PASSWORD, DATABASE, DATABASE_PORT
REDIS_HOST, REDIS_PORT, REDIS_PASSWORD
R2_ACCESS_TOKEN, R2_ACCESS_KEY, R2_URL, R2_BUCKET_NAME, R2_FILE_PATH
HASH_KEY                          # JWT signing secret
ORIGIN                            # CORS allowed origin
GOOGLE_SERVICE_CLIENT_ID, GOOGLE_SERVICE_SERCET
AWS_EMAIL_SENDER, AWS_ACCESS_KEY, AWS_ACCESS_SECRET
LOG_TAIL_SOURCE_KEY
COMMIT_SHA                        # Git SHA shown at /system-version
```

### CORS Origins

`past-exam.zeabur.app`, `past-exam.ntpu.cc`, `past-exam.ntpu.xyz`, `localhost:3000`, `localhost:8080`
