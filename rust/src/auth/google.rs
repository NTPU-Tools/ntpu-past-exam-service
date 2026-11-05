use serde_json::Value;

use crate::config::Config;
use crate::error::{ApiError, ApiResult};

pub async fn exchange(
    client: &reqwest::Client,
    config: &Config,
    code: &str,
    redirect_uri: &str,
) -> ApiResult<Value> {
    let response = client
        .post(&config.google_token_url)
        .form(&[
            (
                "client_id",
                config.google_client_id.clone().unwrap_or_default(),
            ),
            (
                "client_secret",
                config.google_client_secret.clone().unwrap_or_default(),
            ),
            ("code", code.to_string()),
            ("redirect_uri", redirect_uri.to_string()),
            ("grant_type", "authorization_code".to_string()),
        ])
        .send()
        .await
        .map_err(ApiError::internal)?;
    let data: Value = response.json().await.map_err(ApiError::internal)?;
    let access_token = data
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::forbidden("Forbidden"))?;

    let info = client
        .get(&config.google_userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(ApiError::internal)?;
    let user_data: Value = info.json().await.map_err(ApiError::internal)?;
    if user_data.get("hd").and_then(Value::as_str) != Some("gm.ntpu.edu.tw") {
        return Err(ApiError::forbidden("Forbidden"));
    }
    Ok(user_data)
}
