import { createKeydock } from "keydock-sdk";

import {
  readConfig,
  requireAuth,
  requireBucketId,
  requireKey,
} from "../../../src/browser-config.js";
import { captureKeydockError } from "../../../src/sdk-test-helpers.js";
import {
  appendLog,
  mountE2eApp,
  renderError,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

mountE2eApp({
  title: "Error Mapping",
  description:
    "Real server errors are normalized into the SDK error classes the browser app receives.",
  steps: [
    { id: "missing-key-result", label: "Missing Key" },
    { id: "missing-key-code", label: "Missing Key Code" },
    { id: "missing-key-detail", label: "Missing Key Detail" },
    { id: "invalid-bucket-result", label: "Invalid Bucket" },
    { id: "invalid-bucket-code", label: "Invalid Bucket Code" },
    { id: "invalid-bucket-detail", label: "Invalid Bucket Detail" },
  ],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const bucketId = requireBucketId(config);
    const auth = requireAuth(config);
    const missingKey = requireKey(config, "missingKey");
    const invalidBucketId = requireKey(config, "invalidBucketId");
    const client = createKeydock({ baseUrl: config.url, auth });

    setText("bucket-id", bucketId);
    appendLog("triggering missing key error");
    const missing = await captureKeydockError(() => client.bucket(bucketId).getJson(missingKey));
    setStep("missing-key-result", "done", `${missing.name}:${missing.status}`);
    setStep("missing-key-code", "done", String(missing.code));
    setStep("missing-key-detail", "done", missing.detail);
    setText("error-name", missing.name);
    setText("error-status", String(missing.status));
    setText("error-detail", missing.detail);

    appendLog("triggering invalid bucket error");
    const invalidBucket = await captureKeydockError(() =>
      client.bucket(invalidBucketId).getText("anything"),
    );
    setStep("invalid-bucket-result", "done", `${invalidBucket.name}:${invalidBucket.status}`);
    setStep("invalid-bucket-code", "done", String(invalidBucket.code));
    setStep("invalid-bucket-detail", "done", invalidBucket.detail);

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
