import { createKeydock, KeydockError } from "keydock-sdk";
import type { CreateBucketInput } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import type { BucketCredentials } from "../../../src/browser-config.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

type SeedMode = "anonymous" | "secret" | "none";

type MatrixRow = {
  id: string;
  label: string;
  input: (credentials: BucketCredentials) => CreateBucketInput;
  seed: SeedMode;
};

const rows: MatrixRow[] = [
  {
    id: "no-keys",
    label: "No Keys",
    seed: "anonymous",
    input: (credentials) => ({
      email: `no-keys-${credentials.email}`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "read-only",
    label: "Read Key Only",
    seed: "anonymous",
    input: (credentials) => ({
      email: `read-only-${credentials.email}`,
      readKey: `${credentials.readKey}-only`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "write-only",
    label: "Write Key Only",
    seed: "none",
    input: (credentials) => ({
      email: `write-only-${credentials.email}`,
      writeKey: `${credentials.writeKey}-only`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "secret-read",
    label: "Secret + Read",
    seed: "secret",
    input: (credentials) => ({
      email: `secret-read-${credentials.email}`,
      secretKey: `${credentials.secretKey}-secret-read`,
      readKey: `${credentials.readKey}-secret-read`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "secret-write",
    label: "Secret + Write",
    seed: "secret",
    input: (credentials) => ({
      email: `secret-write-${credentials.email}`,
      secretKey: `${credentials.secretKey}-secret-write`,
      writeKey: `${credentials.writeKey}-secret-write`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "read-write",
    label: "Read + Write",
    seed: "none",
    input: (credentials) => ({
      email: `read-write-${credentials.email}`,
      readKey: `${credentials.readKey}-read-write`,
      writeKey: `${credentials.writeKey}-read-write`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "signing-only",
    label: "Signing Key Only",
    seed: "anonymous",
    input: (credentials) => ({
      email: `signing-only-${credentials.email}`,
      signingKey: `${credentials.signingKey ?? credentials.secretKey}-signing-only`,
      defaultTtlSeconds: 0,
    }),
  },
];

const steps = rows.flatMap((row) => [
  { id: `${row.id}-read`, label: `${row.label} Read` },
  { id: `${row.id}-list`, label: `${row.label} List` },
  { id: `${row.id}-write`, label: `${row.label} Write` },
  { id: `${row.id}-delete`, label: `${row.label} Delete` },
]);

mountE2eApp({
  title: "Anonymous Auth Matrix",
  description:
    "A browser mini app verifies anonymous access for every documented static-key combination.",
  bucketId: "not-created",
  steps,
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const credentials = requireCredentials(config);
    const anonymous = createKeydock({ baseUrl: config.url });

    for (const row of rows) {
      const input = row.input(credentials);
      const created = await anonymous.buckets.create(input);
      setText("bucket-id", created.id);

      const anonymousBucket = anonymous.bucket(created.id);
      const cleanupClient =
        input.secretKey === undefined
          ? undefined
          : createKeydock({ baseUrl: config.url, auth: input.secretKey });

      try {
        if (row.seed === "anonymous") {
          await anonymousBucket.setText("read-target", "ok");
        } else if (row.seed === "secret" && cleanupClient !== undefined) {
          await cleanupClient.bucket(created.id).setText("read-target", "ok");
        }

        setStep(
          `${row.id}-read`,
          "done",
          await captureResult(() => anonymousBucket.getText("read-target")),
        );
        setStep(`${row.id}-list`, "done", await captureResult(() => anonymousBucket.listKeys()));
        setStep(
          `${row.id}-write`,
          "done",
          await captureResult(() => anonymousBucket.setText("write-target", "v")),
        );
        setStep(
          `${row.id}-delete`,
          "done",
          await captureResult(() => anonymousBucket.delete("write-target")),
        );
      } finally {
        if (cleanupClient !== undefined) {
          await cleanupClient.buckets.delete(created.id);
        }
      }
    }

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

async function captureResult(operation: () => Promise<unknown>): Promise<string> {
  try {
    await operation();
    return "ok";
  } catch (error) {
    if (error instanceof KeydockError) {
      return `${error.name}:${error.status}`;
    }
    throw error;
  }
}
