import { expect, test } from "@playwright/test";

import {
  createClient,
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";
import type { KeydockE2eConfig } from "../src/browser-config.js";

test.describe("basic SDK browser roundtrip", () => {
  let cleanupTarget: CreatedBucketFixture | undefined;

  test.afterEach(async ({ page }) => {
    if (cleanupTarget !== undefined) {
      const bucketId = await page
        .getByTestId("bucket-id")
        .textContent()
        .catch(() => undefined);
      if (bucketId !== undefined && bucketId !== null && bucketId !== "not-created") {
        cleanupTarget = {
          id: bucketId,
          credentials: cleanupTarget.credentials,
        };
      }
    }
    await deleteBucketBestEffort(cleanupTarget);
    cleanupTarget = undefined;
  });

  test("creates a bucket and performs key operations through the installed SDK", async ({
    page,
  }) => {
    const credentials = uniqueBucketData("basic");
    cleanupTarget = {
      id: "not-created",
      credentials,
    };
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      credentials,
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/basic/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText("bucket created");
    await expect(page.getByTestId("set-result")).toHaveText("values written");
    await expect(page.getByTestId("get-result")).toHaveText("hello from browser");
    await expect(page.getByTestId("json-result")).toHaveText("Ana:true");
    await expect(page.getByTestId("exists-result")).toHaveText("true");
    await expect(page.getByTestId("list-result")).toContainText("message");
    await expect(page.getByTestId("list-result")).toContainText("profile");
    await expect(page.getByTestId("delete-result")).toHaveText("message deleted");
    await expect(page.getByTestId("post-delete-exists-result")).toHaveText("false");

    const bucketId = await page.getByTestId("bucket-id").textContent();
    if (bucketId !== null && bucketId !== "not-created") {
      await expect(createClient(credentials.secretKey).buckets.exists(bucketId)).resolves.toBe(
        false,
      );
    }
  });
});
