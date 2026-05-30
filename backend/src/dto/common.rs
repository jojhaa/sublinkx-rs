use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T> {
    pub code: &'static str,
    pub data: T,
}
