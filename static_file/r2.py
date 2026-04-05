import os

import boto3
from botocore.config import Config
from dotenv import load_dotenv

s3_config = Config(retries={"max_attempts": 10, "mode": "standard"})

load_dotenv()

s3 = boto3.resource(
    service_name="s3",
    endpoint_url=os.getenv("R2_URL"),
    aws_access_key_id=os.getenv("R2_ACCESS_KEY"),
    aws_secret_access_key=os.getenv("R2_ACCESS_TOKEN"),
    config=s3_config,
)

r2 = boto3.client(
    service_name="s3",
    endpoint_url=os.getenv("R2_URL"),
    aws_access_key_id=os.getenv("R2_ACCESS_KEY"),
    aws_secret_access_key=os.getenv("R2_ACCESS_TOKEN"),
    region_name="auto",
    config=s3_config,
)


def list_all_files():
    file_list = []
    meta = r2.list_objects_v2(Bucket=os.getenv("R2_BUCKET_NAME"))

    try:
        for obj in meta["Contents"]:
            file_list.append(f"{os.getenv('R2_FILE_PATH')}/{obj['Key']}")
        return {"count": meta["KeyCount"], "data": file_list}
    except KeyError:
        return {"count": 0, "data": file_list}
