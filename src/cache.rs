use axum::http::Uri;
use redis::aio::ConnectionManager;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Redis-backed response cache with per-namespace generation counters.
/// Redis failures are fail-open: the request is served uncached.
#[derive(Clone)]
pub struct Cache {
    redis: ConnectionManager,
    prefix: String,
    clear_legacy: bool,
}

fn digest(uri: &Uri) -> String {
    let key = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_else(|| uri.path());
    hex::encode(Sha256::digest(key.as_bytes()))
}

impl Cache {
    pub fn new(redis: ConnectionManager, prefix: String, clear_legacy: bool) -> Self {
        Cache {
            redis,
            prefix,
            clear_legacy,
        }
    }

    fn generation_key(&self, namespace: &str) -> String {
        format!("{}:gen:{}", self.prefix, namespace)
    }

    fn entry_key(
        &self,
        namespace: &str,
        generation: i64,
        user_id: &str,
        route: &str,
        uri: &Uri,
    ) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            self.prefix,
            namespace,
            generation,
            user_id,
            route,
            digest(uri)
        )
    }

    async fn generation(&self, namespace: &str) -> i64 {
        let mut connection = self.redis.clone();
        match redis::cmd("GET")
            .arg(self.generation_key(namespace))
            .query_async::<Option<String>>(&mut connection)
            .await
        {
            Ok(Some(value)) => value.parse().unwrap_or(0),
            _ => 0,
        }
    }

    pub async fn get(
        &self,
        namespace: &str,
        user_id: &str,
        route: &str,
        uri: &Uri,
    ) -> Option<Value> {
        let generation = self.generation(namespace).await;
        let mut connection = self.redis.clone();
        let key = self.entry_key(namespace, generation, user_id, route, uri);
        match redis::cmd("GET")
            .arg(key)
            .query_async::<Option<String>>(&mut connection)
            .await
        {
            Ok(Some(raw)) => serde_json::from_str(&raw).ok(),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(error = %error, "cache read failed");
                None
            }
        }
    }

    pub async fn set(
        &self,
        namespace: &str,
        ttl_seconds: u64,
        user_id: &str,
        route: &str,
        uri: &Uri,
        value: &Value,
    ) {
        let generation = self.generation(namespace).await;
        let key = self.entry_key(namespace, generation, user_id, route, uri);
        let Ok(raw) = serde_json::to_string(value) else {
            return;
        };
        let mut connection = self.redis.clone();
        let result: Result<(), _> = redis::cmd("SET")
            .arg(key)
            .arg(raw)
            .arg("EX")
            .arg(ttl_seconds)
            .query_async(&mut connection)
            .await;
        if let Err(error) = result {
            tracing::warn!(error = %error, "cache write failed");
        }
    }

    pub async fn invalidate(&self, namespace: &str) {
        let mut connection = self.redis.clone();
        let result: Result<i64, _> = redis::cmd("INCR")
            .arg(self.generation_key(namespace))
            .query_async(&mut connection)
            .await;
        if let Err(error) = result {
            tracing::warn!(error = %error, namespace, "cache invalidation failed");
            return;
        }
        if self.clear_legacy {
            self.clear_legacy_namespace(namespace).await;
        }
    }

    async fn clear_legacy_namespace(&self, namespace: &str) {
        let pattern = format!("fastapi-cache:v2:{namespace}:*");
        let mut connection = self.redis.clone();
        let mut cursor = 0u64;
        loop {
            let result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(1000)
                .query_async(&mut connection)
                .await;
            let (next, keys) = match result {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(error = %error, namespace, "legacy cache scan failed");
                    return;
                }
            };
            if !keys.is_empty() {
                let _: Result<i64, _> = redis::cmd("UNLINK")
                    .arg(&keys)
                    .query_async(&mut connection)
                    .await;
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
    }
}
