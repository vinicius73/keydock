import { createKeydock } from "keydock-sdk";

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
  setAttr,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

mountE2eApp({
  title: "Transaction Timeline",
  description:
    "The SDK posts a transaction from the browser and then verifies the resulting server state.",
  steps: [
    { id: "transaction-result", label: "Transaction" },
    { id: "first-read-result", label: "First Read" },
    { id: "second-read-result", label: "Second Read" },
    { id: "deleted-read-result", label: "Deleted Read" },
    { id: "counter-result", label: "Counter" },
  ],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const bucketId = requireBucketId(config);
    const auth = requireAuth(config);
    const firstKey = requireKey(config, "firstKey");
    const secondKey = requireKey(config, "secondKey");
    const deletedKey = requireKey(config, "deletedKey");
    const counterKey = requireKey(config, "counterKey");
    const bucket = createKeydock({ baseUrl: config.url, auth }).bucket(bucketId);

    setText("bucket-id", bucketId);
    appendLog("posting transaction");
    await bucket.transaction([
      { set: firstKey, value: "one" },
      { set: secondKey, value: { value: "two" } },
      { delete: deletedKey },
    ]);
    setStep("transaction-result", "done", "transaction committed");

    const first = await bucket.getText(firstKey);
    setStep("first-read-result", "done", first);

    const second = await bucket.getJson<{ value: string }>(secondKey);
    setStep("second-read-result", "done", second.value);

    const deleted = await bucket.getTextOrNull(deletedKey);
    setStep("deleted-read-result", "done", String(deleted));

    const counter = await bucket.increment(counterKey, 1);
    setAttr("counter-result", "data-counter-raw", counter.raw);
    setAttr("counter-result", "data-counter-kind", counter.kind);
    if (counter.kind === "integer" && counter.number !== undefined) {
      setAttr("counter-result", "data-counter-number", String(counter.number));
    }
    setStep("counter-result", "done", counter.raw);

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
