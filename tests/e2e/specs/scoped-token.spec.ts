import { expect, test } from "@playwright/test";

import {
  createBucket,
  createClient,
  createScopedToken,
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { randomKey, uniqueBucketData } from "../support/test-data.js";
import type { KeydockE2eConfig } from "../src/browser-config.js";

test.describe("scoped token SDK browser flow", () => {
  let fixture: CreatedBucketFixture | undefined;

  test.afterEach(async () => {
    await deleteBucketBestEffort(fixture);
    fixture = undefined;
  });

  test("allows prefixed reads and rejects out-of-scope reads", async ({ page }) => {
    fixture = await createBucket(uniqueBucketData("scoped"));
    const scopedKey = randomKey("scope:profile");
    const outsideKey = randomKey("private:profile");
    const adminBucket = createClient(fixture.credentials.secretKey).bucket(fixture.id);

    await adminBucket.setText(scopedKey, "visible-through-token");
    await adminBucket.setText(outsideKey, "not-visible-through-token");

    const token = await createScopedToken(fixture.id, fixture.credentials.secretKey, {
      prefix: "scope:",
      permissions: ["read"],
    });
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      bucketId: fixture.id,
      auth: token,
      keys: {
        scopedKey,
        outsideKey,
      },
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/scoped-token/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("bucket-id")).toHaveText(fixture.id);
    await expect(page.getByTestId("scoped-read-result")).toHaveText("visible-through-token");
    await expect(page.getByTestId("outside-scope-result")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("error-name")).toHaveText("KeydockError");
    await expect(page.getByTestId("error-status")).toHaveText("403");
  });
});
