import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from sql.database import SessionLocal
from departments.models import Department

ORDER_MAP = [
    "中國文學系",
    "應用外語學系",
    "歷史學系",
    "休閒運動管理學系",
    "會計學系",
    "統計學系",
    "企業管理學系",
    "金融與合作經營學系",
    "不動產與城鄉環境學系",
    "公共行政暨政策學系",
    "財政學系",
    "法律學系",
    "經濟學系",
    "社會學系",
    "社會工作學系",
    "資訊工程學系",
    "通訊工程學系",
    "電機工程學系",
]


def main():
    db = SessionLocal()
    try:
        updated = 0
        not_found = []

        for index, name in enumerate(ORDER_MAP):
            dept = db.query(Department).filter(Department.name == name).first()
            if dept:
                dept.sort_order = index
                updated += 1
                print(f"  [{index}] {name}")
            else:
                not_found.append(name)
                print(f"  [WARN] Not found: {name}")

        db.commit()

        print(f"\nUpdated: {updated} / {len(ORDER_MAP)}")
        if not_found:
            print(f"Not found: {not_found}")

        others = (
            db.query(Department)
            .filter(Department.name.notin_(ORDER_MAP))
            .all()
        )
        if others:
            print(f"\nOther departments (sort_order remains 999):")
            for dept in others:
                print(f"  - {dept.name}")
    finally:
        db.close()


if __name__ == "__main__":
    main()
