# @keydock/sdk

Official TypeScript SDK for Keydock.

The SDK is ESM-only, Fetch-based, and designed for modern runtimes: Node.js 22+,
Bun, Deno, browsers, and edge environments. It uses Ky internally while exposing a
Keydock-oriented API for buckets, keys, tokens, and transactions.

## Install

```sh
npm install @keydock/sdk
```

For Bun:

```sh
bun add @keydock/sdk
```

For Deno and JSR:

```ts
import { createKeydock } from "jsr:@keydock/sdk";
```

## Node.js Or Bun

```ts
import { createKeydock } from "@keydock/sdk";

const auth = process.env.KEYDOCK_SECRET_KEY;
if (!auth) {
  throw new Error("KEYDOCK_SECRET_KEY is required");
}

const keydock = createKeydock({
  baseUrl: "https://keydock.example.com",
  auth,
});

const bucket = keydock.bucket("bucket-id");

await bucket.setJson("users/42", { name: "Ana" }, { ttlSeconds: 3600 });

const user = await bucket.getJson<{ name: string }>("users/42");
console.log(user.name);
```

## Deno

```ts
import { createKeydock } from "jsr:@keydock/sdk";

const auth = Deno.env.get("KEYDOCK_SECRET_KEY");
if (!auth) {
  throw new Error("KEYDOCK_SECRET_KEY is required");
}

const keydock = createKeydock({
  baseUrl: "https://keydock.example.com",
  auth,
});

const bucket = keydock.bucket("bucket-id");
const profile = await bucket.getJson<{ name: string }>("profiles/ana");
```

## Browser

Browsers are not trusted environments. Do not ship bucket `secretKey`, root keys,
long-lived admin credentials, or signing keys to browser code.

Use short-lived scoped tokens minted by your application backend:

```ts
import { createKeydock } from "@keydock/sdk";

const keydock = createKeydock({
  baseUrl: "https://keydock.example.com",
  auth: async () => {
    const response = await fetch("/api/keydock-token");
    if (!response.ok) {
      throw new Error("Failed to get Keydock token");
    }
    return response.text();
  },
});

const bucket = keydock.bucket("bucket-id");
const profile = await bucket.getJson<{ displayName: string }>("public/profile.json");
```

Browser-safe credential patterns:

- Anonymous access for public buckets.
- Read-only keys for public data.
- Carefully scoped write-only keys for ingestion flows.
- Temporary scoped tokens with limited prefix, permissions, and TTL.

Credentials that are not browser-safe:

- Bucket `secretKey`.
- Root/server secrets.
- Long-lived admin credentials.
- Signing keys.

## Custom Ky Instance

Advanced users can provide a Ky instance for application-specific hooks, tracing,
timeouts, or custom `fetch` implementations.

```ts
import ky from "ky";
import { createKeydock } from "@keydock/sdk";

const http = ky.create({
  timeout: 5000,
  retry: {
    limit: 2,
    methods: ["get", "head"],
  },
  hooks: {
    beforeRequest: [
      ({ request }) => {
        request.headers.set("x-client", "web-app");
      },
    ],
  },
});

const keydock = createKeydock({
  baseUrl: "https://keydock.example.com",
  auth: () => getKeydockToken(),
  http,
});
```

## Error Handling

Keydock HTTP error responses are normalized into `KeydockError`.

```ts
import { KeydockError, createKeydock } from "@keydock/sdk";

const keydock = createKeydock({ baseUrl, auth });
const bucket = keydock.bucket("bucket-id");

try {
  await bucket.getJson("missing");
} catch (error) {
  if (error instanceof KeydockError && error.status === 404) {
    // Missing keys can be handled explicitly.
  } else {
    throw error;
  }
}
```

Other SDK errors:

- `KeydockTimeoutError` for request timeouts.
- `KeydockNetworkError` for network failures.
- `KeydockValidationError` for invalid local inputs such as empty keys, invalid TTLs,
  zero counter increments, null transaction values, or attempts to clear the bucket secret key.

## Retry Behaviour

Default retries are conservative:

- `GET` and `HEAD` may retry transient failures.
- `PUT`, `PATCH`, `POST`, and `DELETE` do not retry by default.
- Counter increments and transactions are never retried by the SDK default path.

This avoids hiding network uncertainty around writes. A network failure after a
successful write, increment, or transaction can be indistinguishable from a failure
before the server applied the operation.

## TTL Semantics

Write methods accept `ttlSeconds` and send it as the server's `ttl` query
parameter:

```ts
await bucket.setText("session/123", "active", { ttlSeconds: 900 });
```

Rules:

- `ttlSeconds` must be a finite integer.
- `ttlSeconds` must be greater than or equal to `0`.
- Per-write `ttlSeconds` overrides the bucket default when present.
- Bucket `defaultTtlSeconds: 0` means no default expiry.

## Key API

```ts
await bucket.setText("message", "hello");
await bucket.setJson("users/42", { name: "Ana" });
await bucket.setBytes("avatar", new Uint8Array([1, 2, 3]));

const text = await bucket.getText("message");
const user = await bucket.getJson<{ name: string }>("users/42");
const bytes = await bucket.getBytes("avatar");

const maybeUser = await bucket.getJsonOrNull<{ name: string }>("users/missing");
const exists = await bucket.exists("message");
```

Keys are logical strings. The SDK percent-encodes them before placing them in HTTP
paths, so callers should not pre-encode keys.

## Listing And Transactions

```ts
const keys = await bucket.listKeys({ prefix: "users/" });
const entries = await bucket.listEntries({ prefix: "users/" });

await bucket.transaction([
  { set: "users/42/name", value: "Ana", ttlSeconds: 3600 },
  { delete: "users/42/tmp" },
]);
```

`listKeys` returns `string[]`. `listEntries` converts the server's JSON tuple shape
into `{ key, value }` objects.

## Bucket Administration And Tokens

```ts
const created = await keydock.buckets.create({
  email: "admin@example.com",
  secretKey: "server-side-admin-secret",
  readKey: "read-only",
  writeKey: "write-only",
  signingKey: "signing-secret",
  defaultTtlSeconds: 604800,
});

const policy = await keydock.buckets.getPolicy(created.id);

await keydock.buckets.updatePolicy(created.id, {
  readKey: "new-read-key",
  writeKey: null,
  defaultTtlSeconds: 0,
});

const token = await keydock.bucket(created.id).tokens.create({
  prefix: "public/",
  permissions: ["read", "enumerate"],
  ttlSeconds: 900,
});
```

Policy fields use camelCase in TypeScript and are converted to the server's
snake_case wire contract internally.
