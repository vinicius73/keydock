import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("TTL and counters golden SDK browser coverage", () => {
  let cleanupTarget: CreatedBucketFixture | undefined;

  test.afterEach(async ({ page }) => {
    if (cleanupTarget !== undefined) {
      const bucketId = await page
        .getByTestId("bucket-id")
        .textContent()
        .catch(() => undefined);
      if (
        bucketId !== undefined &&
        bucketId !== null &&
        bucketId !== "not-created"
      ) {
        cleanupTarget = {
          id: bucketId,
          credentials: cleanupTarget.credentials,
        };
      }
    }
    await deleteBucketBestEffort(cleanupTarget);
    cleanupTarget = undefined;
  });

  test("covers TTL expiry, default TTLs, and counter edge cases", async ({
    page,
  }) => {
    const credentials = uniqueBucketData("ttl-counters");
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

    await page.goto("/apps/ttl-counters/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText(
      "bucket created",
    );
    await expect(page.getByTestId("setText-ttl-expires")).toHaveText("null");
    await expect(page.getByTestId("setJson-ttl-expires")).toHaveText(
      "undefined",
    );
    await expect(page.getByTestId("setBytes-ttl-expires")).toHaveText("null");
    await expect(page.getByTestId("ttl-zero-no-expiry")).toHaveText("v");
    await expect(page.getByTestId("ttl-renewal")).toHaveText("v");
    await expect(page.getByTestId("default-ttl-604800")).toHaveText("604800");
    await expect(page.getByTestId("default-ttl-zero")).toHaveText("v");
    await expect(page.getByTestId("ttl-expired-excluded-from-list")).toHaveText(
      "[]",
    );
    await expect(page.getByTestId("counter-from-zero-int")).toHaveText(
      "raw:1,kind:integer,number:1",
    );
    await expect(page.getByTestId("counter-negative")).toHaveText(
      "raw:-3,kind:integer,number:-3",
    );
    await expect(page.getByTestId("counter-add-int")).toHaveText(
      "raw:15,kind:integer,number:15",
    );
    await expect(page.getByTestId("counter-int-plus-float")).toHaveText(
      "raw:10.5,kind:float,number:10.5",
    );
    await expect(page.getByTestId("counter-bigint-safe")).toHaveText(
      "raw:42,kind:integer,number:42",
    );
    await expect(page.getByTestId("counter-bigint-unsafe")).toHaveText(
      "raw:9007199254740993,kind:integer",
    );
    await expect(page.getByTestId("counter-zero-rejected")).toHaveText(
      "KeydockValidationError",
    );
    await expect(page.getByTestId("counter-nan-rejected")).toHaveText(
      "KeydockValidationError",
    );
    await expect(page.getByTestId("counter-non-numeric")).toHaveText(
      "KeydockError:400",
    );
    await expect(page.getByTestId("counter-with-ttl")).toHaveText("null");
  });
});
