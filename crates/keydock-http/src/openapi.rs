use utoipa::OpenApi;

use crate::error::{ErrorBody, ErrorDetail};
use crate::routes::buckets::{self, CreateBucketForm, ListBucketParams, UpdatePolicyForm};
use crate::routes::health::{self, HealthResponse};
use crate::routes::keys::{self, TtlQuery};
use crate::routes::tokens::{self, AccessTokenResponse, CreateTokenForm};
use crate::routes::txn::{self, TxnDelete, TxnItem, TxnRequest, TxnSet};

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health_check,
        health::readiness_check,
        buckets::create_bucket,
        buckets::list_bucket,
        buckets::update_policy,
        buckets::delete_bucket,
        buckets::get_bucket_reserved_openapi,
        tokens::create_token,
        keys::get_key,
        keys::put_key_openapi,
        keys::delete_key,
        keys::patch_key,
        txn::execute_txn,
    ),
    components(schemas(
        HealthResponse,
        ErrorBody,
        ErrorDetail,
        TtlQuery,
        TxnRequest,
        TxnItem,
        TxnSet,
        TxnDelete,
        CreateBucketForm,
        UpdatePolicyForm,
        ListBucketParams,
        CreateTokenForm,
        AccessTokenResponse,
    ))
)]
pub struct ApiDoc;
