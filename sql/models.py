# pylint: skip-file

from bulletins.models import Bulletin
from courses.models import Course
from departments.models import Department
from posts.models import Post, PostFile
from users.models import User, UserDepartment

from .database import Base
