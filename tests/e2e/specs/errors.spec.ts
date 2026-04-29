import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  createBucket,
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { randomKey, uniqueBucketData } from "../support/test-data.js";

test.describe("SDK browser error mapping", () => {
  let fixture: CreatedBucketFixture | undefined;

  test.afterEach(async () => {
    await deleteBucketBestEffort(fixture);
    fixture = undefined;
  });

  test("normalizes real server errors into KeydockError", async ({ page }) => {
    fixture = await createBucket(uniqueBucketData("errors"));
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      bucketId: fixture.id,
      auth: fixture.credentials.secretKey,
      keys: {
        missingKey: randomKey("missing"),
        invalidBucketId: randomKey("missing-bucket"),
      },
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/errors/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("missing-key-result")).toHaveText("KeydockError:404");
    await expect(page.getByTestId("missing-key-code")).toHaveText("404");
    await expect(page.getByTestId("missing-key-detail")).not.toHaveText("");
    await expect(page.getByTestId("invalid-bucket-result")).toHaveText("KeydockError:404");
    await expect(page.getByTestId("invalid-bucket-code")).toHaveText("404");
    await expect(page.getByTestId("invalid-bucket-detail")).not.toHaveText("");
    await expect(page.getByTestId("error-name")).toHaveText("KeydockError");
    await expect(page.getByTestId("error-status")).toHaveText("404");
    await expect(page.getByTestId("error-detail")).not.toHaveText("");
  });
});
