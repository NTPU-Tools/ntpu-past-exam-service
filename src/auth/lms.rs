use axum::http::header::COOKIE;
use scraper::node::Node;
use scraper::{ElementRef, Html, Selector};

use crate::error::{ApiError, ApiResult};

pub struct LmsUser {
    pub readable_name: String,
    pub department: String,
    pub email: Option<String>,
}

fn internal(message: &str) -> ApiError {
    ApiError::internal(message)
}

fn find_element_containing_text<'a>(document: &'a Html, needle: &str) -> Option<ElementRef<'a>> {
    let selector = Selector::parse("*").ok()?;
    for element in document.select(&selector) {
        for child in element.children() {
            if let Node::Text(text) = child.value() {
                if text.text.as_ref() == needle {
                    return Some(element);
                }
            }
        }
    }
    None
}

/// Parses the LMS login response and returns the `date1` redirect token.
pub(crate) fn parse_login_html(html: &str) -> ApiResult<String> {
    let document = Html::parse_document(html);
    let h3 = Selector::parse("h3").expect("valid selector");
    if document.select(&h3).next().is_some() {
        return Err(ApiError::bad_request("Incorrect username or password"));
    }
    let body_selector = Selector::parse("body").expect("valid selector");
    let onload = document
        .select(&body_selector)
        .next()
        .and_then(|body| body.value().attr("onload"))
        .ok_or_else(|| internal("LMS login page changed: missing body onload"))?;
    onload
        .split("window.open('../univer/query_all_course.login2?date1=")
        .nth(1)
        .and_then(|rest| rest.split("','_top')").next())
        .map(str::to_string)
        .ok_or_else(|| internal("LMS login page changed: missing redirect timestamp"))
}

/// Returns `(readable_name, department)` from the course info page.
pub(crate) fn parse_course_info_html(html: &str) -> ApiResult<(String, String)> {
    let document = Html::parse_document(html);
    let parent = find_element_containing_text(&document, "(選課說明：")
        .ok_or_else(|| internal("LMS course info page changed: marker not found"))?;
    let span = Selector::parse("span").expect("valid selector");
    let spans: Vec<ElementRef<'_>> = parent.select(&span).collect();
    let department = spans
        .get(1)
        .map(|span| span.text().collect::<String>())
        .ok_or_else(|| internal("LMS course info page changed: missing department"))?;
    let readable_name = spans
        .get(3)
        .map(|span| span.text().collect::<String>())
        .ok_or_else(|| internal("LMS course info page changed: missing name"))?;
    Ok((readable_name, department))
}

/// Returns the email value; `Ok(None)` when the input exists without a value
/// (the reference implementation maps that to SQL NULL).
pub(crate) fn parse_contact_html(html: &str) -> ApiResult<Option<String>> {
    let document = Html::parse_document(html);
    let email_selector = Selector::parse("input[type=email]").expect("valid selector");
    match document.select(&email_selector).next() {
        Some(input) => Ok(input.value().attr("value").map(str::to_string)),
        None => Err(internal("LMS contact page changed: missing email input")),
    }
}

pub async fn get_user_info(
    client: &reqwest::Client,
    base_url: &str,
    username: &str,
    password: &str,
) -> ApiResult<LmsUser> {
    let response = client
        .post(format!("{base_url}/pls/pm/stud_system.login"))
        .form(&[
            ("stud_num", username),
            ("passwd", password),
            ("x", "109"),
            ("y", "15"),
        ])
        .send()
        .await
        .map_err(ApiError::internal)?;

    let mut cookies = Vec::new();
    for value in response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
    {
        if let Ok(value) = value.to_str() {
            if let Some(pair) = value.split(';').next() {
                cookies.push(pair.to_string());
            }
        }
    }
    let cookie_header = cookies.join("; ");
    let login_html = response.text().await.map_err(ApiError::internal)?;
    let date1 = parse_login_html(&login_html)?;

    let info_url = format!("{base_url}/pls/univer/query_all_course.judge?func=10&date1={date1}");
    let info_html = client
        .get(info_url)
        .header(COOKIE, &cookie_header)
        .send()
        .await
        .map_err(ApiError::internal)?
        .text()
        .await
        .map_err(ApiError::internal)?;
    let (readable_name, department) = parse_course_info_html(&info_html)?;

    let contact_url = format!("{base_url}/pls/univer/query_all_course.judge?func=18&date1={date1}");
    let contact_html = client
        .get(contact_url)
        .header(COOKIE, &cookie_header)
        .send()
        .await
        .map_err(ApiError::internal)?
        .text()
        .await
        .map_err(ApiError::internal)?;
    let email = parse_contact_html(&contact_html)?;

    Ok(LmsUser {
        readable_name,
        department,
        email,
    })
}
