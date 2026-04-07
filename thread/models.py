from sqlalchemy import (
    Boolean,
    Column,
    ForeignKey,
    Integer,
    String,
    Text,
    UniqueConstraint,
)

from sql.database import Base, BaseColumn


# NOTE: DB-level CASCADE deletes are in effect on thread_comments (via thread_id and
# parent_comment_id FKs) and on thread_likes/comment_likes. If relationship() definitions
# are added in the future, they MUST include passive_deletes=True to prevent SQLAlchemy
# from issuing SET NULL before the DB CASCADE fires.


class Thread(Base, BaseColumn):
    __tablename__ = "threads"
    __table_args__ = {"mysql_charset": "utf8mb4", "mysql_collate": "utf8mb4_0900_ai_ci"}

    title = Column(String(256))
    content = Column(Text)
    image_url = Column(String(1024), nullable=True)
    owner_id = Column(String(256), ForeignKey("users.id"), index=True)
    course_id = Column(String(256), ForeignKey("courses.id"), index=True)
    is_anonymous = Column(Boolean, default=False)
    like_count = Column(Integer, default=0)


class ThreadComment(Base, BaseColumn):
    __tablename__ = "thread_comments"
    __table_args__ = {"mysql_charset": "utf8mb4", "mysql_collate": "utf8mb4_0900_ai_ci"}

    thread_id = Column(
        String(256), ForeignKey("threads.id", ondelete="CASCADE"), index=True
    )
    parent_comment_id = Column(
        String(256),
        ForeignKey("thread_comments.id", ondelete="CASCADE"),
        index=True,
        nullable=True,
    )
    content = Column(Text)
    owner_id = Column(String(256), ForeignKey("users.id"), index=True)
    is_anonymous = Column(Boolean, default=False)
    like_count = Column(Integer, default=0)


class ThreadLike(Base, BaseColumn):
    __tablename__ = "thread_likes"
    __table_args__ = (
        UniqueConstraint("thread_id", "user_id", name="unique_thread_like"),
    )

    thread_id = Column(
        String(256), ForeignKey("threads.id", ondelete="CASCADE"), index=True
    )
    user_id = Column(String(256), ForeignKey("users.id"), index=True)


class CommentLike(Base, BaseColumn):
    __tablename__ = "comment_likes"
    __table_args__ = (
        UniqueConstraint("comment_id", "user_id", name="unique_comment_like"),
    )

    comment_id = Column(
        String(256), ForeignKey("thread_comments.id", ondelete="CASCADE"), index=True
    )
    user_id = Column(String(256), ForeignKey("users.id"), index=True)
