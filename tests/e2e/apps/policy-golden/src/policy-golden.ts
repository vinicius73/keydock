import { createKeydock, KeydockError, KeydockValidationError } from "keydock-sdk";
import type { BucketPolicy } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const steps = [
  { id: "create-policy-shape", label: "Create Policy Shape" },
  { id: "create-no-default-ttl", label: "No Default TTL" },
  { id: "create-default-ttl-zero", label: "Default TTL Zero" },
  { id: "policy-secrets-absent", label: "Secrets Absent" },
  { id: "update-rotate-read-key", label: "Rotate Read Key" },
  { id: "update-clear-write-key", label: "Clear Write Key" },
  { id: "update-clear-read-key", label: "Clear Read Key" },
  { id: "update-clear-signing", label: "Clear Signing" },
  { id: "update-rotate-signing", label: "Same Signing" },
  { id: "update-secret-key-null", label: "Secret Null" },
  { id: "update-default-ttl", label: "Update TTL" },
  { id: "update-clear-default-ttl", label: "Clear TTL" },
  { id: "bucket-exists-true", label: "Bucket Exists" },
  { id: "bucket-exists-false", label: "Bucket Missing" },
  { id: "bucket-delete", label: "Bucket Delete" },
  { id: "policy-non-admin-forbidden", label: "Policy Non-admin" },
  { id: "policy-non-admin-head", label: "Head Non-admin" },
] as const;

mountE2eApp({
  title: "Policy Golden",
  description:
    "A browser mini app verifies bucket policy projection, updates, exists, delete, and admin-only checks through the SDK.",
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
      defaultTtlSeconds: 3_600,
    };
    const created = await anonymous.buckets.create(
      credentials.signingKey === undefined
        ? createInput
        : {
            ...createInput,
            signingKey: credentials.signingKey,
          },
    );
    setText("bucket-id", created.id);

    const policy = await admin.buckets.getPolicy(created.id);
    setStep("create-policy-shape", "done", policySummary(policy));
    setStep("policy-secrets-absent", "done", hasRawSecrets(policy) ? "exposed" : "absent");

    await withTemporaryBucket(
      config.url,
      {
        email: `no-default-${credentials.email}`,
        secretKey: `${credentials.secretKey}-no-default`,
      },
      async (client, bucketId) => {
        const current = await client.buckets.getPolicy(bucketId);
        setStep("create-no-default-ttl", "done", String(current.defaultTtlSeconds));
      },
    );

    await withTemporaryBucket(
      config.url,
      {
        email: `zero-${credentials.email}`,
        secretKey: `${credentials.secretKey}-zero`,
        defaultTtlSeconds: 0,
      },
      async (client, bucketId) => {
        const current = await client.buckets.getPolicy(bucketId);
        setStep("create-default-ttl-zero", "done", String(current.defaultTtlSeconds));
      },
    );

    const bucket = admin.bucket(created.id);
    await bucket.setText("read-check", "ok");
    const oldRead = createKeydock({ baseUrl: config.url, auth: credentials.readKey }).bucket(
      created.id,
    );
    const newReadKey = `${credentials.readKey}-rotated`;
    await admin.buckets.updatePolicy(created.id, { readKey: newReadKey });
    const oldReadError = await captureKeydockError(() => oldRead.getText("read-check"));
    const newRead = createKeydock({ baseUrl: config.url, auth: newReadKey }).bucket(created.id);
    const newReadValue = await newRead.getText("read-check");
    setStep(
      "update-rotate-read-key",
      "done",
      oldReadError.status === 401 && newReadValue === "ok" ? "ok" : "error",
    );

    await admin.buckets.updatePolicy(created.id, { writeKey: null });
    setStep(
      "update-clear-write-key",
      "done",
      String((await admin.buckets.getPolicy(created.id)).hasWriteKey),
    );

    await admin.buckets.updatePolicy(created.id, { readKey: null });
    setStep(
      "update-clear-read-key",
      "done",
      await anonymous.bucket(created.id).getText("read-check"),
    );

    await admin.buckets.updatePolicy(created.id, { signingKey: null });
    const clearedSigning = await admin.buckets.getPolicy(created.id);
    setStep(
      "update-clear-signing",
      "done",
      `sign:${String(clearedSigning.hasSigningKey)},gen:${clearedSigning.signingKeyGeneration}`,
    );

    await withTemporaryBucket(
      config.url,
      {
        email: `same-signing-${credentials.email}`,
        secretKey: `${credentials.secretKey}-same-signing`,
        signingKey: `${credentials.signingKey}-same`,
      },
      async (client, bucketId) => {
        await client.buckets.updatePolicy(bucketId, {
          signingKey: `${credentials.signingKey}-same`,
        });
        setStep(
          "update-rotate-signing",
          "done",
          String((await client.buckets.getPolicy(bucketId)).signingKeyGeneration),
        );
      },
    );

    const secretKeyNull = await captureAnyError(() =>
      admin.buckets.updatePolicy(created.id, { secretKey: null as unknown as string }),
    );
    setStep("update-secret-key-null", "done", secretKeyNull);

    await admin.buckets.updatePolicy(created.id, { defaultTtlSeconds: 120 });
    setStep(
      "update-default-ttl",
      "done",
      String((await admin.buckets.getPolicy(created.id)).defaultTtlSeconds),
    );

    await admin.buckets.updatePolicy(created.id, { defaultTtlSeconds: null });
    setStep(
      "update-clear-default-ttl",
      "done",
      String((await admin.buckets.getPolicy(created.id)).defaultTtlSeconds),
    );

    setStep("bucket-exists-true", "done", String(await admin.buckets.exists(created.id)));
    setStep("bucket-exists-false", "done", String(await admin.buckets.exists("__never__")));

    await withTemporaryBucket(
      config.url,
      {
        email: `non-admin-${credentials.email}`,
        secretKey: `${credentials.secretKey}-non-admin`,
        readKey: `${credentials.readKey}-non-admin`,
      },
      async (_client, bucketId) => {
        const readClient = createKeydock({
          baseUrl: config.url,
          auth: `${credentials.readKey}-non-admin`,
        });
        const policyError = await captureKeydockError(() => readClient.buckets.getPolicy(bucketId));
        setStep("policy-non-admin-forbidden", "done", `${policyError.name}:${policyError.status}`);

        const headError = await captureKeydockError(() => readClient.buckets.exists(bucketId));
        setStep("policy-non-admin-head", "done", `${headError.name}:${headError.status}`);
      },
    );

    await admin.buckets.delete(created.id);
    setStep("bucket-delete", "done", String(await admin.buckets.exists(created.id)));

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

function policySummary(policy: BucketPolicy): string {
  return [
    `secret:${String(policy.hasSecretKey)}`,
    `read:${String(policy.hasReadKey)}`,
    `write:${String(policy.hasWriteKey)}`,
    `sign:${String(policy.hasSigningKey)}`,
    `gen:${policy.signingKeyGeneration}`,
    `ttl:${String(policy.defaultTtlSeconds)}`,
  ].join(",");
}

function hasRawSecrets(policy: BucketPolicy): boolean {
  const raw = policy as Record<string, unknown>;
  return (
    "secretKey" in raw ||
    "readKey" in raw ||
    "writeKey" in raw ||
    "signingKey" in raw ||
    "secret_key" in raw ||
    "read_key" in raw ||
    "write_key" in raw ||
    "signing_key" in raw
  );
}

async function withTemporaryBucket(
  baseUrl: string,
  input: {
    email: string;
    secretKey: string;
    readKey?: string;
    signingKey?: string;
    defaultTtlSeconds?: number;
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
