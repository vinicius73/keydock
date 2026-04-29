import { createKeydock } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import {
  bucketCreateInput,
  captureKeydockError,
  createPublicBucket,
  publicBucketSecretKey,
} from "../../../src/sdk-test-helpers.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const steps = [
  { id: "create-result", label: "Create" },
  { id: "admin-read", label: "Admin Read" },
  { id: "admin-write", label: "Admin Write" },
  { id: "admin-list", label: "Admin List" },
  { id: "admin-delete", label: "Admin Delete" },
  { id: "readKey-read", label: "Read Key Read" },
  { id: "readKey-write", label: "Read Key Write" },
  { id: "readKey-list", label: "Read Key List" },
  { id: "readKey-delete", label: "Read Key Delete" },
  { id: "writeKey-read", label: "Write Key Read" },
  { id: "writeKey-write", label: "Write Key Write" },
  { id: "writeKey-list", label: "Write Key List" },
  { id: "anon-read", label: "Anonymous Read" },
  { id: "anon-write", label: "Anonymous Write" },
  { id: "wrong-cred", label: "Wrong Credential" },
  { id: "missing-bucket", label: "Missing Bucket" },
  { id: "public-anon-read", label: "Public Read" },
  { id: "public-anon-write", label: "Public Write" },
  { id: "public-anon-list", label: "Public List" },
  { id: "public-anon-delete", label: "Public Delete" },
] as const;

mountE2eApp({
  title: "Auth Matrix",
  description:
    "A browser mini app verifies every SDK credential level against the backend authorization matrix.",
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
    const admin = createKeydock({
      baseUrl: config.url,
      auth: credentials.secretKey,
    });
    const readClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.readKey,
    });
    const writeClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.writeKey,
    });
    const wrongClient = createKeydock({
      baseUrl: config.url,
      auth: "wrong-credential",
    });

    setStep("create-result", "running", "creating bucket");
    const created = await anonymous.buckets.create(
      bucketCreateInput(credentials, { defaultTtlSeconds: 0 }),
    );
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");

    const adminBucket = admin.bucket(created.id);
    const readBucket = readClient.bucket(created.id);
    const writeBucket = writeClient.bucket(created.id);
    const anonymousBucket = anonymous.bucket(created.id);
    const wrongBucket = wrongClient.bucket(created.id);

    await adminBucket.setText("read-target", "ok");
    setStep(
      "admin-read",
      "done",
      (await adminBucket.getText("read-target")) === "ok" ? "ok" : "error",
    );

    await adminBucket.setText("admin-write", "v");
    setStep("admin-write", "done", "ok");

    await adminBucket.listKeys();
    setStep("admin-list", "done", "ok");

    await adminBucket.setText("admin-delete", "v");
    await adminBucket.delete("admin-delete");
    setStep("admin-delete", "done", "ok");

    setStep(
      "readKey-read",
      "done",
      (await readBucket.getText("read-target")) === "ok" ? "ok" : "error",
    );
    const readWrite = await captureKeydockError(() => readBucket.setText("read-denied", "v"));
    setStep("readKey-write", "done", `${readWrite.name}:${readWrite.status}`);

    await readBucket.listKeys();
    setStep("readKey-list", "done", "ok");

    const readDelete = await captureKeydockError(() => readBucket.delete("read-target"));
    setStep("readKey-delete", "done", `${readDelete.name}:${readDelete.status}`);

    const writeRead = await captureKeydockError(() => writeBucket.getText("read-target"));
    setStep("writeKey-read", "done", `${writeRead.name}:${writeRead.status}`);

    await writeBucket.setText("write-allowed", "v");
    setStep("writeKey-write", "done", "ok");

    const writeList = await captureKeydockError(() => writeBucket.listKeys());
    setStep("writeKey-list", "done", `${writeList.name}:${writeList.status}`);

    const anonRead = await captureKeydockError(() => anonymousBucket.getText("read-target"));
    setStep("anon-read", "done", `${anonRead.name}:${anonRead.status}`);

    const anonWrite = await captureKeydockError(() => anonymousBucket.setText("anon-denied", "v"));
    setStep("anon-write", "done", `${anonWrite.name}:${anonWrite.status}`);

    const wrongRead = await captureKeydockError(() => wrongBucket.getText("read-target"));
    setStep("wrong-cred", "done", `${wrongRead.name}:${wrongRead.status}`);

    const missingBucket = await captureKeydockError(() =>
      admin.bucket(`${created.id}-missing`).getText("anything"),
    );
    setStep("missing-bucket", "done", `${missingBucket.name}:${missingBucket.status}`);

    await runPublicBucketChecks(config.url, credentials);
    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

async function runPublicBucketChecks(
  baseUrl: string,
  credentials: { email: string; secretKey: string },
): Promise<void> {
  const anonymous = createKeydock({ baseUrl });
  const publicSecretKey = publicBucketSecretKey(credentials);
  const publicBucketId = await createPublicBucket(baseUrl, credentials);
  const admin = createKeydock({ baseUrl, auth: publicSecretKey });
  const adminBucket = admin.bucket(publicBucketId);
  const anonymousBucket = anonymous.bucket(publicBucketId);

  try {
    await adminBucket.setText("public-read", "ok");
    setStep("public-anon-read", "done", await anonymousBucket.getText("public-read"));

    await anonymousBucket.setText("public-write", "ok");
    setStep("public-anon-write", "done", "ok");

    await anonymousBucket.listKeys();
    setStep("public-anon-list", "done", "ok");

    const publicDelete = await captureKeydockError(() => anonymousBucket.delete("public-read"));
    setStep("public-anon-delete", "done", `${publicDelete.name}:${publicDelete.status}`);
  } finally {
    await admin.buckets.delete(publicBucketId);
  }
}
