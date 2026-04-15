use axum_test::TestServer;
use bytes::Bytes;
use serde::Serialize;

#[derive(Serialize)]
pub struct CreateBucketForm {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_ttl: Option<u64>,
}

pub async fn create_bucket(server: &TestServer, form: &CreateBucketForm) -> String {
    let body = serde_urlencoded::to_string(form).expect("encode form");
    let response = server
        .post("/")
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await;
    response.assert_status_ok();
    response.text()
}
