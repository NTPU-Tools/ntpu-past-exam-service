import requests
from bs4 import BeautifulSoup
from fastapi import HTTPException


def get_lms_readable_name(username: str, password: str):
    response = requests.post(
        "https://cof.ntpu.edu.tw/pls/pm/stud_system.login",
        data={
            "stud_num": username,
            "passwd": password,
            "x": "109",
            "y": "15",
        },
        timeout=10,
    )
    cookies = response.cookies
    soup = BeautifulSoup(response.text, "html.parser")
    if soup.find("h3"):
        raise HTTPException(status_code=400, detail="Incorrect username or password")
    timestamp = (
        soup.find("body")
        .get("onload")
        .split("window.open('../univer/query_all_course.login2?date1=")[1]
        .split("','_top')")[0]
    )

    user_info_link = f"https://cof.ntpu.edu.tw/pls/univer/query_all_course.judge?func=10&date1={timestamp}"

    user_info_page = requests.get(user_info_link, cookies=cookies, timeout=10)
    user_info_soup = BeautifulSoup(user_info_page.text, "html.parser")
    return user_info_soup.find(text="(選課說明：").parent.find_all("span")[3].text
