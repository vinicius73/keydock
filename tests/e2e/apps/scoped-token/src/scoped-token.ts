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

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
