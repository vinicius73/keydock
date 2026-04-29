import { createKeydock, KeydockError, KeydockValidationError } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const steps = [
  { id: "create-result", label: "Create" },
  { id: "token-read-within", label: "Read Within" },
  { id: "token-read-outside", label: "Read Outside" },
  { id: "token-read-no-write", label: "Read No Write" },
  { id: "token-write-within", label: "Write Within" },
  { id: "token-write-no-read", label: "Write No Read" },
  { id: "token-write-no-delete", label: "Write No Delete" },
  { id: "token-delete-within", label: "Delete Within" },
  { id: "token-delete-outside", label: "Delete Outside" },
  { id: "token-enumerate-within", label: "Enumerate Within" },
  { id: "token-enumerate-no-read", label: "Enumerate No Read" },
  { id: "token-expired", label: "Expired" },
  { id: "token-wrong-bucket", label: "Wrong Bucket" },
  { id: "token-post-rotation", label: "Post Rotation" },
  { id: "token-new-after-rotation", label: "New After Rotation" },
  { id: "token-no-signing-key", label: "No Signing Key" },
  { id: "sdk-empty-prefix", label: "Empty Prefix" },
  { id: "sdk-zero-ttl", label: "Zero TTL" },
  { id: "sdk-empty-permissions", label: "Empty Permissions" },
] as const;

mountE2eApp({
  title: "Tokens Golden",
  description:
    "A browser mini app verifies scoped-token permissions, prefix enforcement, expiry, rotation, and SDK validation.",
  bucketId: "not-created",
  steps: [...steps],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const credentials = requireCredentials(config);
    const anonymous = createKeydock({ baseUrl: config.url });
    const admin = createKeydock({ baseUrl: config.url, auth: credentials.secretKey });

    const createInput = {
      email: credentials.email,
      secretKey: credentials.secretKey,
      readKey: credentials.readKey,
      writeKey: credentials.writeKey,
      defaultTtlSeconds: 0,
    };
    const created = await anonymous.buckets.create(
      credentials.signingKey === undefined
        ? {
            ...createInput,
            signingKey: `${credentials.secretKey}-signing`,
          }
        : {
            ...createInput,
            signingKey: credentials.signingKey,
          },
    );
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");

    const adminBucket = admin.bucket(created.id);
    await adminBucket.setText("scope:k1", "ok");
    await adminBucket.setText("other:k", "outside");

    const readToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["read"],
      ttlSeconds: 900,
    });
    const readBucket = tokenBucket(config.url, created.id, readToken.accessToken);
    setStep("token-read-within", "done", await readBucket.getText("scope:k1"));

    const readOutside = await captureKeydockError(() => readBucket.getText("other:k"));
    setStep("token-read-outside", "done", `${readOutside.name}:${readOutside.status}`);

    const readNoWrite = await captureKeydockError(() =>
      readBucket.setText("scope:read-write", "v"),
    );
    setStep("token-read-no-write", "done", `${readNoWrite.name}:${readNoWrite.status}`);

    const writeToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["write"],
      ttlSeconds: 900,
    });
    const writeBucket = tokenBucket(config.url, created.id, writeToken.accessToken);
    await writeBucket.setText("scope:write", "ok");
    setStep("token-write-within", "done", "ok");

    const writeNoRead = await captureKeydockError(() => writeBucket.getText("scope:write"));
    setStep("token-write-no-read", "done", `${writeNoRead.name}:${writeNoRead.status}`);

    const writeNoDelete = await captureKeydockError(() => writeBucket.delete("scope:write"));
    setStep("token-write-no-delete", "done", `${writeNoDelete.name}:${writeNoDelete.status}`);

    await adminBucket.setText("scope:delete", "gone");
    await adminBucket.setText("other:delete", "kept");
    const deleteToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["delete"],
      ttlSeconds: 900,
    });
    const deleteBucket = tokenBucket(config.url, created.id, deleteToken.accessToken);
    await deleteBucket.delete("scope:delete");
    setStep("token-delete-within", "done", "ok");

    const deleteOutside = await captureKeydockError(() => deleteBucket.delete("other:delete"));
    setStep("token-delete-outside", "done", `${deleteOutside.name}:${deleteOutside.status}`);

    const enumerateToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["enumerate"],
      ttlSeconds: 900,
    });
    const enumerateBucket = tokenBucket(config.url, created.id, enumerateToken.accessToken);
    const enumerated = await enumerateBucket.listKeys({ prefix: "scope:" });
    setStep("token-enumerate-within", "done", enumerated.includes("scope:k1") ? "ok" : "missing");

    const enumerateNoRead = await captureKeydockError(() => enumerateBucket.getText("scope:k1"));
    setStep("token-enumerate-no-read", "done", `${enumerateNoRead.name}:${enumerateNoRead.status}`);

    const expiredToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["read"],
      ttlSeconds: 1,
    });
    await sleep(2_000);
    const expired = await captureKeydockError(() =>
      tokenBucket(config.url, created.id, expiredToken.accessToken).getText("scope:k1"),
    );
    setStep("token-expired", "done", `${expired.name}:${expired.status}`);

    await withTemporaryBucket(
      config.url,
      {
        email: `wrong-bucket-${credentials.email}`,
        secretKey: `${credentials.secretKey}-wrong-bucket`,
      },
      async (_client, bucketId) => {
        const wrongBucket = tokenBucket(config.url, bucketId, readToken.accessToken);
        const wrongBucketError = await captureKeydockError(() => wrongBucket.getText("scope:k1"));
        setStep(
          "token-wrong-bucket",
          "done",
          `${wrongBucketError.name}:${wrongBucketError.status}`,
        );
      },
    );

    const beforeRotationToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["read"],
      ttlSeconds: 900,
    });
    await admin.buckets.updatePolicy(created.id, {
      signingKey: `${credentials.signingKey}-rotated`,
    });
    const rotated = await captureKeydockError(() =>
      tokenBucket(config.url, created.id, beforeRotationToken.accessToken).getText("scope:k1"),
    );
    setStep("token-post-rotation", "done", `${rotated.name}:${rotated.status}`);

    const afterRotationToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["read"],
      ttlSeconds: 900,
    });
    const afterRotation = await tokenBucket(
      config.url,
      created.id,
      afterRotationToken.accessToken,
    ).getText("scope:k1");
    setStep("token-new-after-rotation", "done", afterRotation);

    await withTemporaryBucket(
      config.url,
      {
        email: `no-signing-${credentials.email}`,
        secretKey: `${credentials.secretKey}-no-signing`,
      },
      async (client, bucketId) => {
        const noSigningError = await captureKeydockError(() =>
          client.bucket(bucketId).tokens.create({
            prefix: "scope:",
            permissions: ["read"],
            ttlSeconds: 900,
          }),
        );
        setStep("token-no-signing-key", "done", `${noSigningError.name}:${noSigningError.status}`);
      },
    );

    setStep(
      "sdk-empty-prefix",
      "done",
      await captureAnyError(() =>
        adminBucket.tokens.create({ prefix: "", permissions: ["read"], ttlSeconds: 900 }),
      ),
    );
    setStep(
      "sdk-zero-ttl",
      "done",
      await captureAnyError(() =>
        adminBucket.tokens.create({ prefix: "scope:", permissions: ["read"], ttlSeconds: 0 }),
      ),
    );
    setStep(
      "sdk-empty-permissions",
      "done",
      await captureAnyError(() =>
        adminBucket.tokens.create({ prefix: "scope:", permissions: [], ttlSeconds: 900 }),
      ),
    );

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

function tokenBucket(baseUrl: string, bucketId: string, accessToken: string) {
  return createKeydock({ baseUrl, auth: accessToken }).bucket(bucketId);
}

async function withTemporaryBucket(
  baseUrl: string,
  input: {
    email: string;
    secretKey: string;
  },
  operation: (client: ReturnType<typeof createKeydock>, bucketId: string) => Promise<void>,
): Promise<void> {
  const anonymous = createKeydock({ baseUrl });
  const created = await anonymous.buckets.create(input);
  const client = createKeydock({ baseUrl, auth: input.secretKey });
  try {
    await operation(client, created.id);
  } finally {
    await client.buckets.delete(created.id);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function captureKeydockError(operation: () => Promise<unknown>): Promise<KeydockError> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return error;
    }
    throw error;
  }

  throw new Error("expected operation to fail with KeydockError");
}

async function captureAnyError(operation: () => Promise<unknown>): Promise<string> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return `${error.name}:${error.status}`;
    }
    if (error instanceof KeydockValidationError) {
      return error.name;
    }
    throw error;
  }

  throw new Error("expected operation to fail");
}
