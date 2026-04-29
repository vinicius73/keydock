import { randomUUID } from "node:crypto";

export type BucketData = {
  email: string;
  secretKey: string;
  readKey: string;
  writeKey: string;
  signingKey: string;
};

export function uniqueBucketData(prefix: string): BucketData {
  const suffix = `${prefix}-${randomUUID()}`;
  return {
    email: `${suffix}@e2e.test`,
    secretKey: `sec-${suffix}`,
    readKey: `read-${suffix}`,
    writeKey: `write-${suffix}`,
    signingKey: `sign-${suffix}`,
  };
}

export function randomKey(prefix: string): string {
  return `${prefix}-${randomUUID()}`;
}
