# 北大考古題 NTPU Past Exam Backend

## Tech Stack

- **Framework**: [Fast api](https://fastapi.tiangolo.com/)
- **Monitor**: [Better Stack](https://betterstack.com/)
- **Static File**: Cloudflare R2
- **Database**: PlanetScale


## Start dev server
1. Create a ".env" file in root folder. It should contain following key:
```
DATABASE_HOST=
DATABASE_USERNAME=
DATABASE_PASSWORD=
DATABASE=

R2_ACCESS_TOKEN=
R2_ACCESS_KEY=
R2_URL=
R2_BUCKET_NAME=
R2_FILE_PATH=

AWS_EMAIL_SENDER=
AWS_ACCESS_KEY=
AWS_ACCESS_SECRET=

ORIGIN=
HASH_KEY=
LOG_TAIL_SOURCE_KEY=
#
REDIS_CONNECTION_STRING=
#
REDIS_HOST=
REDIS_PORT=
REDIS_PASSWORD=

GOOGLE_SERVICE_SERCET=
GOOGLE_SERVICE_CLIENT_ID=
```

You should get the value from the code owner.
2. Install dependancies:
```shell
poetry install
```

3. Start dev server
```shell
poetry run uvicorn main:app --reload 
```
