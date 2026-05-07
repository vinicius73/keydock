import { createKeydock } from "keydock-sdk";
import type { CreateBucketInput } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import type { BucketCredentials } from "../../../src/browser-config.js";
import { captureKeydockOutcome } from "../../../src/sdk-test-helpers.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

type SeedMode = "anonymous" | "secret" | "none";
type CleanupSafeBucketInput = CreateBucketInput & { secretKey: string };

type MatrixRow = {
  id: string;
  label: string;
  input: (credentials: BucketCredentials) => CleanupSafeBucketInput;
  seed: SeedMode;
};

const rows: MatrixRow[] = [
  {
    id: "secret-only",
    label: "Secret Key Only",
    seed: "anonymous",
    input: (credentials) => ({
      email: `secret-only-${credentials.email}`,
      secretKey: `${credentials.secretKey}-secret-only`,
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
    id: "all-three",
    label: "Secret + Read + Write",
    seed: "none",
    input: (credentials) => ({
      email: `all-three-${credentials.email}`,
      secretKey: `${credentials.secretKey}-all-three`,
      readKey: `${credentials.readKey}-all-three`,
      writeKey: `${credentials.writeKey}-all-three`,
      defaultTtlSeconds: 0,
    }),
  },
  {
    id: "secret-signing",
    label: "Secret + Signing",
    seed: "anonymous",
    input: (credentials) => ({
      email: `secret-signing-${credentials.email}`,
      secretKey: `${credentials.secretKey}-secret-signing`,
      signingKey: `${credentials.signingKey ?? credentials.secretKey}-secret-signing`,
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
    "A browser mini app verifies cleanup-safe anonymous access for documented static-key combinations.",
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
      const cleanupClient = createKeydock({
        baseUrl: config.url,
        auth: input.secretKey,
      });

      try {
        if (row.seed === "anonymous") {
          await anonymousBucket.setText("read-target", "ok");
        } else if (row.seed === "secret") {
          await cleanupClient.bucket(created.id).setText("read-target", "ok");
        }

        setStep(
          `${row.id}-read`,
          "done",
          await captureKeydockOutcome(() => anonymousBucket.getText("read-target")),
        );
        setStep(
          `${row.id}-list`,
          "done",
          await captureKeydockOutcome(() => anonymousBucket.listKeys()),
        );
        setStep(
          `${row.id}-write`,
          "done",
          await captureKeydockOutcome(() => anonymousBucket.setText("write-target", "v")),
        );
        setStep(
          `${row.id}-delete`,
          "done",
          await captureKeydockOutcome(() => anonymousBucket.delete("write-target")),
        );
      } finally {
        await cleanupClient.buckets.delete(created.id);
      }
    }

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
