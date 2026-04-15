use axum_test::TestServer;
use bytes::Bytes;
use serde::Serialize;

#[derive(Serialize)]
pub struct CreateTokenForm {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    pub permissions: String,
    pub ttl: i64,
}

#[derive(Serialize)]
pub struct UpdatePolicyForm {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
}

pub async fn create_token(
    server: &TestServer,
    bucket_id: &str,
    bearer: &str,
    form: &CreateTokenForm,
) -> axum_test::TestResponse {
    let body = serde_urlencoded::to_string(form).expect("encode form");
    server
        .post(&format!("/{bucket_id}/tokens/"))
        .authorization_bearer(bearer)
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await
}

pub async fn patch_policy(
    server: &TestServer,
    bucket_id: &str,
    bearer: &str,
    form: &UpdatePolicyForm,
) -> axum_test::TestResponse {
    let body = serde_urlencoded::to_string(form).expect("encode form");
    server
        .patch(&format!("/{bucket_id}"))
        .authorization_bearer(bearer)
        .bytes(Bytes::from(body))
        .content_type("application/x-www-form-urlencoded")
        .await
}
