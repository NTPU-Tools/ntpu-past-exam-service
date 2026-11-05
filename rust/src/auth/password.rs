use crate::error::{ApiError, ApiResult};

const BCRYPT_COST: u32 = 12;

fn truncated(password: &str) -> &[u8] {
    let bytes = password.as_bytes();
    &bytes[..bytes.len().min(72)]
}

pub fn hash(password: &str) -> ApiResult<String> {
    bcrypt::hash(truncated(password), BCRYPT_COST).map_err(ApiError::internal)
}

pub fn verify(password: &str, hashed: &str) -> bool {
    bcrypt::verify(truncated(password), hashed).unwrap_or(false)
}
