# keydock

`keydock` is a multi-tenant HTTP key-value service written in Rust.

It stores keys inside isolated buckets and exposes a small HTTP API for bucket
management, key reads and writes, TTLs, scoped temporary tokens, atomic
transactions, health checks, Prometheus metrics, and OpenAPI documentation.

The project is inspired by [KVdb](https://kvdb.io/): a simple HTTP key-value
store API built around buckets, key operations, counters, and lightweight
credential-based access.

## Features

- Multi-tenant buckets with independent credentials.
- HTTP key-value API with plain text, JSON, numeric, and raw byte values.
- Per-bucket admin, read, and write credentials.
- Anonymous access for public buckets.
- Scoped temporary tokens with prefix-based permissions.
- Per-write TTL and per-bucket default TTL.
- Atomic multi-key transactions.
- Counter increment operations.
- Prometheus metrics.
- OpenAPI JSON and Swagger UI.

## Quick Start

Initialize an instance directory:

```bash
keydock init ./instance
```

Start the server:

```bash
keydock serve -c ./instance/keydock.toml
```

By default, the server listens on:

```text
http://127.0.0.1:8080
```

Product API endpoints are mounted under:

```text
/api/v1
```

Operational and documentation endpoints stay at the root:

```text
GET /health
GET /ready
GET /metrics
GET /api-docs/openapi.json
GET /swagger-ui/
```

When `http.metrics_listen` is configured, `/metrics` is served on that dedicated listener instead of the main HTTP address.

## Configuration

`keydock init <DIR>` creates:

```text
<DIR>/keydock.toml
<DIR>/data/
```

The generated config includes:

- HTTP listen address.
- Data directory.
- Root key used to hash API credentials at rest.
- Garbage collection interval for expired keys.
- Optional rate limiting.

Runtime options:

```bash
keydock serve --config ./instance/keydock.toml
keydock serve --listen 127.0.0.1:8080
keydock serve --data-dir ./instance/data
```

## Authentication

API credentials can be sent using one of these forms:

```http
Authorization: Bearer <credential>
```

```http
Authorization: Basic <base64(username:password)>
```

For Basic auth, the username is used as the credential.

Query parameters are also supported:

```text
?access_token=<credential>
?key=<credential>
```

Credential priority:

1. `Authorization` header.
2. `access_token` query parameter.
3. `key` query parameter.

Bucket credentials:

- `secret_key`: admin access.
- `read_key`: read and list access.
- `write_key`: write access.
- Temporary token: scoped access by key prefix and permissions.

Secret material is never returned by policy APIs. API credentials are hashed
before persistence.

## Resource Model

A bucket is an isolated namespace for keys.

Product API paths use this base URL:

```text
/api/v1
```

A key is addressed as:

```text
/api/v1/{bucket}/{key}
```

Keys are opaque bytes on the server and are percent-decoded from the path.
Clients should percent-encode spaces, slashes, and other reserved characters in
key names.

Important limits:

- Key length: up to 128 bytes.
- Value length: up to 16 KiB.

Values are stored with an inferred kind:

- `Content-Type: application/json` stores JSON.
- UTF-8 text stores plain text.
- Numeric text can be used by counter operations.
- Raw bytes are supported for binary payloads.

TTL can be provided on write operations:

```text
?ttl=<seconds>
```

Buckets may also define a default TTL. When a bucket is created without
`default_ttl`, the hosted default is 7 days (`604800` seconds). Send
`default_ttl=0` when creating a bucket if values should not expire by default.

## Operational Endpoints

### Health

```http
GET /health
```

Returns liveness status.

Example response:

```json
{
  "status": "ok",
  "storage": "ok",
  "version": "0.1.0-alpha"
}
```

### Readiness

```http
GET /ready
```

Checks whether metadata storage is reachable.

### Metrics

```http
GET /metrics
```

Returns Prometheus metrics using the text exposition format.

### OpenAPI

```http
GET /api-docs/openapi.json
GET /swagger-ui/
```

The OpenAPI document includes product routes under `/api/v1`.

## Bucket API

### Create Bucket

```http
POST /api/v1/
Content-Type: application/x-www-form-urlencoded
```

Form fields:

- `email`: required owner/admin label.
- `secret_key`: optional admin credential.
- `read_key`: optional read credential.
- `write_key`: optional write credential.
- `signing_key`: optional key used to issue temporary tokens.
- `default_ttl`: optional default TTL in seconds.

Example:

```bash
curl -X POST http://127.0.0.1:8080/api/v1/ \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'email=owner@example.com' \
  --data-urlencode 'secret_key=admin-secret' \
  --data-urlencode 'read_key=read-secret' \
  --data-urlencode 'write_key=write-secret' \
  --data-urlencode 'signing_key=token-signing-secret'
```

The response body is the new bucket id as `text/plain`.

### Get Bucket Policy

```http
GET /api/v1/{bucket}
Authorization: Bearer <secret_key>
```

Returns a public projection of the bucket policy. Secret values and key hashes
are not included.

Example response:

```json
{
  "default_ttl": 604800,
  "has_secret_key": true,
  "has_read_key": true,
  "has_write_key": true,
  "has_signing_key": true,
  "signing_key_generation": 0,
  "anonymous_access": {
    "read": false,
    "write": false,
    "enumerate": false,
    "delete": false
  }
}
```

### Check Bucket

```http
HEAD /api/v1/{bucket}
Authorization: Bearer <secret_key>
```

Returns `200 OK` when the bucket exists and the credential has admin access.

### Update Bucket Policy

```http
PATCH /api/v1/{bucket}
Authorization: Bearer <secret_key>
Content-Type: application/json
```

Supported fields:

- `secret_key`
- `read_key`
- `write_key`
- `signing_key`
- `default_ttl`

Patch behavior:

- Missing field: leave unchanged.
- `null`: clear the field, except `secret_key`.
- Value: set or rotate the field.
- Empty strings are rejected.
- Unknown fields are rejected.

Example:

```bash
curl -X PATCH "http://127.0.0.1:8080/api/v1/$BUCKET" \
  -H 'Authorization: Bearer admin-secret' \
  -H 'Content-Type: application/json' \
  -d '{"read_key":"new-read-secret","default_ttl":3600}'
```

Success returns `204 No Content`.

### Delete Bucket

```http
DELETE /api/v1/{bucket}
Authorization: Bearer <secret_key>
```

The trailing-slash form is also supported:

```http
DELETE /api/v1/{bucket}/
```

Success returns `204 No Content`.

### List Keys

```http
GET /api/v1/{bucket}/
```

Query parameters:

- `prefix`: only list keys with this prefix.
- `limit`: maximum number of keys.
- `skip`: number of keys to skip after ordering and expiry filtering.
- `reverse`: when `true`, use reverse lexicographic order.
- `values`: when `true`, include values.
- `format`: `text`, `json`, or `jsonl`.

The response format can also be selected with `Accept`:

- `text/plain`
- `application/json`
- `application/x-ndjson`

Example:

```bash
curl "http://127.0.0.1:8080/api/v1/$BUCKET/?format=json" \
  -H 'Authorization: Bearer read-secret'
```

Example response:

```json
["a", "b", "c"]
```

With values:

```bash
curl "http://127.0.0.1:8080/api/v1/$BUCKET/?values=true&format=json" \
  -H 'Authorization: Bearer read-secret'
```

Example response:

```json
[["key", "value"]]
```

When a temporary token has a key prefix, listing is restricted to that scope. If
the requested `prefix` is incompatible with the token scope, the response is an
empty successful listing.

## Key API

### Write Key

```http
PUT /api/v1/{bucket}/{key}
POST /api/v1/{bucket}/{key}
```

Example:

```bash
curl -X PUT "http://127.0.0.1:8080/api/v1/$BUCKET/message" \
  -H 'Authorization: Bearer write-secret' \
  -H 'Content-Type: text/plain' \
  -d 'hello'
```

With TTL:

```bash
curl -X PUT "http://127.0.0.1:8080/api/v1/$BUCKET/message?ttl=60" \
  -H 'Authorization: Bearer write-secret' \
  -d 'expires in 60 seconds'
```

The stored value is echoed in the response.

### Read Key

```http
GET /api/v1/{bucket}/{key}
```

Example:

```bash
curl "http://127.0.0.1:8080/api/v1/$BUCKET/message" \
  -H 'Authorization: Bearer read-secret'
```

### Check Key

```http
HEAD /api/v1/{bucket}/{key}
```

Returns `200 OK` when the key exists, has not expired, and is visible to the
credential. The body is empty.

### Delete Key

```http
DELETE /api/v1/{bucket}/{key}
```

Example:

```bash
curl -X DELETE "http://127.0.0.1:8080/api/v1/$BUCKET/message" \
  -H 'Authorization: Bearer admin-secret'
```

Success returns `204 No Content`.

### Increment Counter

```http
PATCH /api/v1/{bucket}/{key}
```

The request body must be a signed delta such as `+1`, `-3`, or `+1.5`.

Example:

```bash
curl -X PATCH "http://127.0.0.1:8080/api/v1/$BUCKET/counter" \
  -H 'Authorization: Bearer write-secret' \
  -d '+1'
```

The response body is the new counter value.

## Temporary Tokens

Temporary tokens are issued from buckets that have a `signing_key`.

```http
POST /api/v1/{bucket}/tokens/
Authorization: Bearer <secret_key>
Content-Type: application/x-www-form-urlencoded
```

Form fields:

- `prefix`: required non-empty key prefix.
- `permissions`: comma-separated permissions: `read`, `write`, `enumerate`, `delete`.
- `ttl`: token lifetime in seconds; must be greater than zero.

Example:

```bash
curl -X POST "http://127.0.0.1:8080/api/v1/$BUCKET/tokens/" \
  -H 'Authorization: Bearer admin-secret' \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'prefix=user:42:' \
  --data-urlencode 'permissions=read,write,enumerate' \
  --data-urlencode 'ttl=3600'
```

Example response:

```json
{
  "access_token": "<token>"
}
```

Use the token like any other bearer credential:

```bash
curl "http://127.0.0.1:8080/api/v1/$BUCKET/user:42:name" \
  -H 'Authorization: Bearer <token>'
```

Temporary tokens are restricted to their configured prefix and permissions. A
token minted for `user:42:` cannot access `admin:config`.

## Transactions

Atomic multi-key transactions are available through:

```http
POST /api/v1/{bucket}
Authorization: Bearer <credential>
Content-Type: application/json
```

Request shape:

```json
{
  "txn": [{ "set": "k1", "value": "hello" }, { "delete": "k2" }]
}
```

Example:

```bash
curl -X POST "http://127.0.0.1:8080/api/v1/$BUCKET" \
  -H 'Authorization: Bearer write-secret' \
  -H 'Content-Type: application/json' \
  -d '{
    "txn": [
      { "set": "profile:name", "value": "Ada", "ttl": 3600 },
      { "delete": "profile:old-name" }
    ]
  }'
```

Success returns `204 No Content`.

Transaction value behavior:

- JSON strings are stored as text.
- JSON numbers, booleans, arrays, and objects are stored as JSON.
- `null` values are rejected.
- Each operation is authorized before the transaction is committed.

## Error Responses

Errors use a consistent JSON envelope:

```json
{
  "error": {
    "code": 404,
    "message": "not_found"
  }
}
```

Common messages:

- `bad_request`
- `unauthorized`
- `forbidden`
- `not_found`
- `not_acceptable`
- `method_not_allowed`
- `service_unavailable`
- `internal_error`

## Development

This repository is a Rust workspace.

Main components:

- `apps/keydock`: binary and process startup.
- `crates/keydock-http`: Axum routes, OpenAPI, and HTTP error mapping.
- `crates/keydock-domain`: domain types and validation.
- `crates/keydock-usecase`: application services and ports.
- `crates/keydock-fjall`: Fjall-backed persistence.
- `crates/keydock-config`: CLI and config loading.
- `crates/keydock-testkit`: integration test support.

Run tests:

```bash
cargo test --workspace
```

Run the project QA flow when available:

```bash
just qa
```

Black-box tests (k6) against a real compiled process:

```bash
tests/k6/run-local.sh smoke
tests/k6/run-local.sh all
```

See `tests/k6/README.md` for Docker mode, running against an existing service, and other variations.

## License

AGPL-3.0-only
