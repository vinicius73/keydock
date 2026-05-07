import { createKeydock } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { bucketCreateInput } from "../../../src/sdk-test-helpers.js";
import {
  appendLog,
  mountE2eApp,
  renderError,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

mountE2eApp({
  title: "Basic Roundtrip",
  description:
    "A browser mini app creates a bucket, writes values, lists keys, and deletes data through the installed SDK package.",
  bucketId: "not-created",
  steps: [
    { id: "create-result", label: "Create" },
    { id: "set-result", label: "Write" },
    { id: "get-result", label: "Read Text" },
    { id: "json-result", label: "Read JSON" },
    { id: "bytes-result", label: "Read Bytes" },
    { id: "exists-result", label: "Exists" },
    { id: "list-result", label: "List" },
    { id: "list-entries-result", label: "List Entries" },
    { id: "delete-result", label: "Delete" },
    { id: "post-delete-exists-result", label: "Exists After Delete" },
    { id: "post-delete-null-result", label: "Null After Delete" },
  ],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const credentials = requireCredentials(config);
    const admin = createKeydock({
      baseUrl: config.url,
      auth: credentials.secretKey,
    });
    const anonymous = createKeydock({ baseUrl: config.url });
    const writeClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.writeKey,
    });
    const readClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.readKey,
    });

    setStep("create-result", "running", "creating bucket");
    const created = await anonymous.buckets.create(
      bucketCreateInput(credentials, { defaultTtlSeconds: 0 }),
    );
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");
    appendLog(`created bucket ${created.id}`);

    const writeBucket = writeClient.bucket(created.id);
    const readBucket = readClient.bucket(created.id);
    const adminBucket = admin.bucket(created.id);

    setStep("set-result", "running", "writing values");
    await writeBucket.setText("message", "hello from browser");
    await writeBucket.setJson("profile", { name: "Ana", ok: true });
    await writeBucket.setBytes("blob", new Uint8Array([1, 2, 3]));
    setStep("set-result", "done", "values written");

    const text = await readBucket.getText("message");
    setStep("get-result", "done", text);

    const json = await readBucket.getJson<{ name: string; ok: boolean }>("profile");
    setStep("json-result", "done", `${json.name}:${String(json.ok)}`);

    const bytes = await readBucket.getBytes("blob");
    setStep("bytes-result", "done", bytes.join(","));

    const exists = await readBucket.exists("message");
    setStep("exists-result", "done", String(exists));

    const keys = await adminBucket.listKeys({ reverse: false });
    setStep("list-result", "done", keys.join(","));

    const entries = await adminBucket.listEntries({ prefix: "" });
    setStep("list-entries-result", "done", entries.map((entry) => entry.key).join(","));

    await adminBucket.delete("message");
    setStep("delete-result", "done", "message deleted");

    const existsAfterDelete = await readBucket.exists("message");
    setStep("post-delete-exists-result", "done", String(existsAfterDelete));

    const valueAfterDelete = await readBucket.getTextOrNull("message");
    setStep("post-delete-null-result", "done", String(valueAfterDelete));

    await admin.buckets.delete(created.id);
    appendLog("bucket deleted");
    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
