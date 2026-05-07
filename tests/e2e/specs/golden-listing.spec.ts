import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("listing golden SDK browser coverage", () => {
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

  test("covers listing options, entries, auth, scoped prefixes, and TTL exclusion", async ({
    page,
  }) => {
    const credentials = uniqueBucketData("listing");
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

    await page.goto("/apps/listing-golden/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText("bucket created");
    await expect(page.getByTestId("list-empty")).toHaveText("[]");
    await expect(page.getByTestId("list-lexicographic")).toHaveText("a,b,c");
    await expect(page.getByTestId("list-reverse")).toHaveText("c,b,a");
    await expect(page.getByTestId("list-prefix")).toHaveText("foo:1,foo:2");
    await expect(page.getByTestId("list-limit")).toHaveText("2");
    await expect(page.getByTestId("list-skip")).toHaveText("k1,k2");
    await expect(page.getByTestId("listEntries-text")).toHaveText("msg=hello");
    await expect(page.getByTestId("listEntries-json")).toHaveText("obj=1");
    await expect(page.getByTestId("list-no-enumerate")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("list-anon-restricted")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("list-anon-public")).toHaveText("public:key");
    await expect(page.getByTestId("list-scoped-compatible")).toHaveText("scope:a,scope:b");
    await expect(page.getByTestId("list-scoped-prefix-override")).toHaveText("a:b1");
    await expect(page.getByTestId("list-scoped-incompatible")).toHaveText("[]");
    await expect(page.getByTestId("list-expired-not-shown")).toHaveText("[]");
  });
});
