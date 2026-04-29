import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  createBucket,
  createClient,
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { randomKey, uniqueBucketData } from "../support/test-data.js";

test.describe("transaction SDK browser flow", () => {
  let fixture: CreatedBucketFixture | undefined;

  test.afterEach(async () => {
    await deleteBucketBestEffort(fixture);
    fixture = undefined;
  });

  test("commits set and delete operations and preserves counter results", async ({ page }) => {
    fixture = await createBucket(uniqueBucketData("txn"));
    const firstKey = randomKey("txn:first");
    const secondKey = randomKey("txn:second");
    const deletedKey = randomKey("txn:deleted");
    const counterKey = randomKey("txn:counter");
    const bucket = createClient(fixture.credentials.secretKey).bucket(fixture.id);

    await bucket.setText(deletedKey, "will-be-deleted");

    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      bucketId: fixture.id,
      auth: fixture.credentials.secretKey,
      keys: {
        firstKey,
        secondKey,
        deletedKey,
        counterKey,
      },
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/transactions/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("transaction-result")).toHaveText("transaction committed");
    await expect(page.getByTestId("first-read-result")).toHaveText("one");
    await expect(page.getByTestId("second-read-result")).toHaveText("two");
    await expect(page.getByTestId("deleted-read-result")).toHaveText("null");
    await expect(page.getByTestId("counter-result")).toHaveText("1");
    await expect(page.getByTestId("counter-result")).toHaveAttribute("data-counter-raw", "1");
    await expect(page.getByTestId("counter-result")).toHaveAttribute(
      "data-counter-kind",
      "integer",
    );
    await expect(page.getByTestId("counter-result")).toHaveAttribute("data-counter-number", "1");
  });
});
