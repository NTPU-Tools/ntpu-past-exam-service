use anyhow::Context;
use opendal::services::S3;
use opendal::Operator;

use crate::config::Config;

pub struct Storage {
    operator: Operator,
    file_path: String,
}

impl Storage {
    pub fn new(config: &Config) -> anyhow::Result<Storage> {
        let builder = S3::default()
            .bucket(&config.r2_bucket)
            .endpoint(&config.r2_url)
            .region("auto")
            .access_key_id(&config.r2_access_key)
            .secret_access_key(&config.r2_secret);
        let operator = Operator::new(builder)
            .context("build S3 operator")?
            .finish();
        Ok(Storage {
            operator,
            file_path: config.r2_file_path.clone(),
        })
    }

    pub async fn put(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<(), opendal::Error> {
        self.operator
            .write_with(key, bytes)
            .content_type(content_type)
            .await
            .map(|_| ())
    }

    pub async fn delete(&self, key: &str) {
        if let Err(error) = self.operator.delete(key).await {
            tracing::warn!(error = %error, key, "failed to delete object");
        }
    }

    pub fn url(&self, key: &str) -> String {
        format!("{}/{}", self.file_path, key)
    }

    pub fn key_from_url(&self, url: &str) -> String {
        url.replacen(&format!("{}/", self.file_path), "", 1)
    }
}
