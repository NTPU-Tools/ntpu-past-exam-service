use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};

fn format_ts(dt: DateTime<Utc>, suffix: &str) -> String {
    let base = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
    let micros = dt.timestamp_subsec_micros();
    if micros == 0 {
        format!("{base}{suffix}")
    } else {
        format!("{base}.{micros:06}{suffix}")
    }
}

/// Python `datetime.isoformat()` shape used by every endpoint except `/threads*`.
#[derive(Debug, Clone, Copy)]
pub struct TsOffset(pub Option<DateTime<Utc>>);

impl From<Option<DateTime<Utc>>> for TsOffset {
    fn from(value: Option<DateTime<Utc>>) -> Self {
        TsOffset(value)
    }
}

impl Serialize for TsOffset {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(dt) => serializer.serialize_str(&format_ts(dt, "+00:00")),
            None => serializer.serialize_none(),
        }
    }
}

/// Pydantic v2 shape used by the thread/comment response models.
#[derive(Debug, Clone, Copy)]
pub struct TsZ(pub Option<DateTime<Utc>>);

impl From<Option<DateTime<Utc>>> for TsZ {
    fn from(value: Option<DateTime<Utc>>) -> Self {
        TsZ(value)
    }
}

impl Serialize for TsZ {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(dt) => serializer.serialize_str(&format_ts(dt, "Z")),
            None => serializer.serialize_none(),
        }
    }
}
