# pylint: skip-file

from bulletins.models import Bulletin
from courses.models import Course
from departments.models import Department
from posts.models import Post, PostFile
from thread.models import CommentLike, Thread, ThreadComment, ThreadLike
from users.models import User, UserDepartment, UserPreference

from .database import Base
