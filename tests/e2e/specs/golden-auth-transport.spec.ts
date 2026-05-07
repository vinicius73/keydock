import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("auth transport golden fetch coverage", () => {
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

  test("covers query, Basic, and priority credential transports", async ({ page }) => {
    const credentials = uniqueBucketData("auth-transport");
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

    await page.goto("/apps/auth-transport-fetch/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("transport-access-token")).toHaveText("200:ok");
    await expect(page.getByTestId("transport-key")).toHaveText("200:ok");
    await expect(page.getByTestId("transport-query-priority")).toHaveText("200:ok");
    await expect(page.getByTestId("transport-bearer-wins-query")).toHaveText("200:ok");
    await expect(page.getByTestId("transport-basic-username")).toHaveText("200:ok");
  });
});
