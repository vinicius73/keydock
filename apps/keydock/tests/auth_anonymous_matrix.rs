//! Anonymous access matrix (HTTP integration).

use axum::http::header;
use rstest::rstest;
use serde_json::json;

use keydock_testkit::{BucketSetup, TestContext, api_error_body_json};

macro_rules! assert_read_response {
    ($response:expr, $expected_status:expr) => {
        match $expected_status {
            404 => {
                $response.assert_status_not_found();
                $response.assert_json(&api_error_body_json(404, "not_found"));
            }
            401 => {
                $response.assert_status_unauthorized();
                $response.assert_json(&api_error_body_json(401, "unauthorized"));
            }
            other => panic!("unsupported read status {other}"),
        }
    };
}

macro_rules! assert_enumerate_response {
    ($response:expr, $expected_status:expr) => {
        match $expected_status {
            200 => {
                $response.assert_status_ok();
                $response.assert_json(&json!([]));
            }
            401 => {
                $response.assert_status_unauthorized();
                $response.assert_json(&api_error_body_json(401, "unauthorized"));
            }
            other => panic!("unsupported enumerate status {other}"),
        }
    };
}

macro_rules! assert_write_response {
    ($response:expr, $expected_status:expr) => {
        match $expected_status {
            200 => {
                $response.assert_status_ok();
                $response.assert_header(header::CONTENT_TYPE, "text/plain; charset=utf-8");
                $response.assert_text("value");
            }
            401 => {
                $response.assert_status_unauthorized();
                $response.assert_json(&api_error_body_json(401, "unauthorized"));
            }
            other => panic!("unsupported write status {other}"),
        }
    };
}

macro_rules! assert_delete_response {
    ($response:expr, $expected_status:expr) => {
        match $expected_status {
            204 => {
                $response.assert_status_no_content();
            }
            401 => {
                $response.assert_status_unauthorized();
                $response.assert_json(&api_error_body_json(401, "unauthorized"));
            }
            other => panic!("unsupported delete status {other}"),
        }
    };
}

#[derive(Clone, Copy)]
enum BucketPolicyCase {
    Public,
    SecretOnly,
    ReadOnly,
    WriteOnly,
    SecretAndRead,
    SecretAndWrite,
    ReadAndWrite,
    AllThree,
    SigningOnly,
}

impl BucketPolicyCase {
    fn setup(self) -> BucketSetup {
        match self {
            Self::Public => BucketSetup::public(),
            Self::SecretOnly => BucketSetup::admin("s"),
            Self::ReadOnly => BucketSetup::read_only("r"),
            Self::WriteOnly => BucketSetup::write_only("w"),
            Self::SecretAndRead => BucketSetup {
                read_key: Some("r".into()),
                ..BucketSetup::admin("s")
            },
            Self::SecretAndWrite => BucketSetup {
                write_key: Some("w".into()),
                ..BucketSetup::admin("s")
            },
            Self::ReadAndWrite => BucketSetup::restricted("r", "w"),
            Self::AllThree => BucketSetup {
                read_key: Some("r".into()),
                write_key: Some("w".into()),
                ..BucketSetup::admin("s")
            },
            Self::SigningOnly => BucketSetup::signing_only("sg"),
        }
    }
}

#[rstest]
#[case::public(BucketPolicyCase::Public, 404, 200, 200, 204)]
#[case::secret_only(BucketPolicyCase::SecretOnly, 404, 200, 200, 401)]
#[case::read_only(BucketPolicyCase::ReadOnly, 401, 200, 401, 401)]
#[case::write_only(BucketPolicyCase::WriteOnly, 404, 401, 200, 401)]
#[case::secret_and_read(BucketPolicyCase::SecretAndRead, 401, 200, 401, 401)]
#[case::secret_and_write(BucketPolicyCase::SecretAndWrite, 404, 401, 200, 401)]
#[case::read_and_write(BucketPolicyCase::ReadAndWrite, 401, 401, 401, 401)]
#[case::all_three(BucketPolicyCase::AllThree, 401, 401, 401, 401)]
#[case::signing_only(BucketPolicyCase::SigningOnly, 404, 200, 200, 204)]
#[tokio::test]
async fn anonymous_access_matrix(
    #[case] policy: BucketPolicyCase,
    #[case] expected_read: u16,
    #[case] expected_write: u16,
    #[case] expected_enumerate: u16,
    #[case] expected_delete: u16,
) {
    let ctx = TestContext::new();
    let bid = ctx.create_bucket(policy.setup()).await;

    let read = ctx.server.get(&format!("/api/v1/{bid}/missing")).await;
    assert_read_response!(read, expected_read);

    let enumerate = ctx.server.get(&format!("/api/v1/{bid}/?format=json")).await;
    assert_enumerate_response!(enumerate, expected_enumerate);

    let key_path = format!("/api/v1/{bid}/matrix-key");
    let write = ctx
        .server
        .put(&key_path)
        .text("value")
        .content_type("text/plain; charset=utf-8")
        .await;
    assert_write_response!(write, expected_write);

    let delete = ctx.server.delete(&key_path).await;
    assert_delete_response!(delete, expected_delete);
}
