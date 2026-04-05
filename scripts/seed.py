"""
Seed script for local development.

Usage:
    docker compose exec backend python scripts/seed.py
"""

import os
import sys
import uuid

# Ensure project root is in path when running as `python scripts/seed.py`
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from alembic import command
from alembic.config import Config
from sqlalchemy.orm import Session

from bulletins.models import Bulletin
from courses.models import Course
from departments.models import Department
from posts.models import Post, PostFile
from sql.database import SessionLocal
from users.models import User, UserDepartment


def hash_password(password: str) -> str:
    import bcrypt
    return bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")


def generate_uuid():
    return str(uuid.uuid4())


def run_migrations():
    project_root = os.path.join(os.path.dirname(__file__), "..")
    alembic_cfg = Config(os.path.join(project_root, "alembic.ini"))
    alembic_cfg.set_main_option("script_location", os.path.join(project_root, "alembic"))
    command.upgrade(alembic_cfg, "head")
    print("Migrations applied.")


def seed():
    run_migrations()
    db: Session = SessionLocal()

    # Check if already seeded
    if db.query(User).first():
        print("Database already seeded. Skipping.")
        db.close()
        return

    # --- Users ---
    admin_id = generate_uuid()
    user1_id = generate_uuid()
    user2_id = generate_uuid()

    admin = User(
        id=admin_id,
        email="admin@gm.ntpu.edu.tw",
        username="admin",
        readable_name="管理員",
        hashed_password=hash_password("admin123"),
        school_department="資訊工程學系",
        is_super_user=True,
    )
    user1 = User(
        id=user1_id,
        email="student1@gm.ntpu.edu.tw",
        username="student1",
        readable_name="王小明",
        hashed_password=hash_password("password123"),
        school_department="資訊工程學系",
        is_super_user=False,
    )
    user2 = User(
        id=user2_id,
        email="student2@gm.ntpu.edu.tw",
        username="student2",
        readable_name="李小華",
        hashed_password=hash_password("password123"),
        school_department="電機工程學系",
        is_super_user=False,
    )
    db.add_all([admin, user1, user2])

    # --- Departments ---
    dept_csie_id = generate_uuid()
    dept_ee_id = generate_uuid()
    dept_law_id = generate_uuid()

    dept_csie = Department(
        id=dept_csie_id,
        key="csie",
        name="資訊工程學系",
        is_public=True,
    )
    dept_ee = Department(
        id=dept_ee_id,
        key="ee",
        name="電機工程學系",
        is_public=True,
    )
    dept_law = Department(
        id=dept_law_id,
        key="law",
        name="法律學系",
        is_public=False,
    )
    db.add_all([dept_csie, dept_ee, dept_law])

    # --- User-Department memberships ---
    db.add_all([
        # Admin is admin of all departments
        UserDepartment(
            id=generate_uuid(),
            user_id=admin_id,
            department_id=dept_csie_id,
            status="APPROVED",
            is_department_admin=True,
        ),
        UserDepartment(
            id=generate_uuid(),
            user_id=admin_id,
            department_id=dept_ee_id,
            status="APPROVED",
            is_department_admin=True,
        ),
        UserDepartment(
            id=generate_uuid(),
            user_id=admin_id,
            department_id=dept_law_id,
            status="APPROVED",
            is_department_admin=True,
        ),
        # student1 in CSIE (approved)
        UserDepartment(
            id=generate_uuid(),
            user_id=user1_id,
            department_id=dept_csie_id,
            status="APPROVED",
            is_department_admin=False,
        ),
        # student2 in EE (approved), pending in CSIE
        UserDepartment(
            id=generate_uuid(),
            user_id=user2_id,
            department_id=dept_ee_id,
            status="APPROVED",
            is_department_admin=False,
        ),
        UserDepartment(
            id=generate_uuid(),
            user_id=user2_id,
            department_id=dept_csie_id,
            status="PENDING",
            is_department_admin=False,
        ),
    ])

    # --- Courses ---
    course_ds_id = generate_uuid()
    course_os_id = generate_uuid()
    course_algo_id = generate_uuid()
    course_circuit_id = generate_uuid()
    course_civil_id = generate_uuid()

    courses = [
        Course(id=course_ds_id, name="資料結構", category="必修", department_id=dept_csie_id),
        Course(id=course_os_id, name="作業系統", category="必修", department_id=dept_csie_id),
        Course(id=course_algo_id, name="演算法", category="選修", department_id=dept_csie_id),
        Course(id=course_circuit_id, name="電路學", category="必修", department_id=dept_ee_id),
        Course(id=course_civil_id, name="民法總則", category="必修", department_id=dept_law_id),
    ]
    db.add_all(courses)

    # --- Posts ---
    post1_id = generate_uuid()
    post2_id = generate_uuid()
    post3_id = generate_uuid()
    post4_id = generate_uuid()

    posts = [
        Post(
            id=post1_id,
            title="112-1 資料結構 期中考",
            content="張教授 期中考題",
            owner_id=user1_id,
            course_id=course_ds_id,
            department_id=dept_csie_id,
            status="APPROVED",
            is_anonymous=False,
        ),
        Post(
            id=post2_id,
            title="112-1 資料結構 期末考",
            content="張教授 期末考題",
            owner_id=user1_id,
            course_id=course_ds_id,
            department_id=dept_csie_id,
            status="APPROVED",
            is_anonymous=True,
        ),
        Post(
            id=post3_id,
            title="112-2 作業系統 期中考",
            content="李教授 期中考題",
            owner_id=admin_id,
            course_id=course_os_id,
            department_id=dept_csie_id,
            status="APPROVED",
            is_anonymous=False,
        ),
        Post(
            id=post4_id,
            title="112-1 電路學 期中考",
            content="王教授 期中考",
            owner_id=user2_id,
            course_id=course_circuit_id,
            department_id=dept_ee_id,
            status="PENDING",
            is_anonymous=False,
        ),
    ]
    db.add_all(posts)

    # --- Bulletins ---
    db.add_all([
        Bulletin(
            id=generate_uuid(),
            title="歡迎使用考古題平台",
            content="歡迎大家分享考古題，請遵守使用規範。",
            department_id=dept_csie_id,
        ),
        Bulletin(
            id=generate_uuid(),
            title="新學期公告",
            content="本學期開始接受新的考古題上傳。",
            department_id=dept_ee_id,
        ),
    ])

    db.commit()
    db.close()

    print("Seeded successfully!")
    print()
    print("Login credentials:")
    print("  admin    / admin123     (super user)")
    print("  student1 / password123  (regular user, CSIE)")
    print("  student2 / password123  (regular user, EE)")


if __name__ == "__main__":
    seed()
