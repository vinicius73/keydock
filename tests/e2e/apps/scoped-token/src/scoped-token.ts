import { createKeydock, KeydockError } from "keydock-sdk";

import {
  readConfig,
  requireAuth,
  requireBucketId,
  requireKey,
} from "../../../src/browser-config.js";
import {
  appendLog,
  mountE2eApp,
  renderError,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

mountE2eApp({
  title: "Scoped Token",
  description:
    "A browser-safe token reads only its prefix and receives a real SDK error outside that scope.",
  steps: [
    { id: "scoped-read-result", label: "Scoped Read" },
    { id: "outside-scope-result", label: "Outside Scope" },
    { id: "write-token-write-result", label: "Write Token Write" },
    { id: "write-token-read-result", label: "Write Token Read" },
  ],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const bucketId = requireBucketId(config);
    const token = requireAuth(config);
    const scopedKey = requireKey(config, "scopedKey");
    const outsideKey = requireKey(config, "outsideKey");
    const writeToken = requireKey(config, "writeToken");
    const writeScopedKey = requireKey(config, "writeScopedKey");
    const client = createKeydock({ baseUrl: config.url, auth: token });
    const bucket = client.bucket(bucketId);

    setText("bucket-id", bucketId);
    appendLog("reading prefixed key with scoped token");
    const value = await bucket.getText(scopedKey);
    setStep("scoped-read-result", "done", value);

    try {
      await bucket.getText(outsideKey);
      setStep("outside-scope-result", "error", "unexpected success");
      setStatus("error", "error");
      return;
    } catch (error) {
      if (!(error instanceof KeydockError)) {
        throw error;
      }
      setText("error-name", error.name);
      setText("error-status", String(error.status));
      setText("error-detail", error.detail);
      setStep("outside-scope-result", "done", `${error.name}:${error.status}`);
    }

    const writeBucket = createKeydock({ baseUrl: config.url, auth: writeToken }).bucket(bucketId);
    await writeBucket.setText(writeScopedKey, "written-through-token");
    setStep("write-token-write-result", "done", "ok");

    const writeReadError = await captureKeydockError(() => writeBucket.getText(writeScopedKey));
    setStep("write-token-read-result", "done", `${writeReadError.name}:${writeReadError.status}`);

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
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
