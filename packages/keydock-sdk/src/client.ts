import { BucketHandle } from "./bucket.js";
import { BucketsNamespace } from "./buckets.js";
import { buildKy } from "./internal/http.js";
import type { KeydockOptions } from "./types.js";

export type KeydockClient = {
  bucket(bucketId: string): BucketHandle;
  buckets: BucketsNamespace;
};

export function createKeydock(options: KeydockOptions): KeydockClient {
  const http = buildKy(options);

  return {
    bucket(bucketId: string) {
      return new BucketHandle(bucketId, http);
    },
    buckets: new BucketsNamespace(http),
  };
}
