import { defineConfig, devices, type PlaywrightTestConfig } from "@playwright/test";

type ScreenshotMode = NonNullable<PlaywrightTestConfig["use"]>["screenshot"];
type VideoMode = NonNullable<PlaywrightTestConfig["use"]>["video"];

const env = process.env;
const screenshot = readScreenshotMode(env.E2E_SCREENSHOT);
const video = readVideoMode(env.E2E_VIDEO);

export default defineConfig({
  testDir: "./specs",
  outputDir: "test-results",
  fullyParallel: true,
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  globalSetup: "./support/global-setup.ts",
  globalTeardown: "./support/global-teardown.ts",
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  webServer: {
    command: "bunx vite --host 127.0.0.1 --port 5173",
    url: "http://127.0.0.1:5173/apps/basic/",
    reuseExistingServer: !env.CI,
    timeout: 20_000,
  },
  use: {
    baseURL: "http://127.0.0.1:5173",
    screenshot,
    trace: "retain-on-failure",
    video,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});

function readScreenshotMode(value: string | undefined): ScreenshotMode {
  switch (value) {
    case undefined:
    case "":
    case "0":
    case "false":
    case "off":
      return "off";
    case "1":
    case "true":
    case "failure":
    case "only-on-failure":
      return "only-on-failure";
    case "always":
    case "on":
      return "on";
    default:
      throw new Error(
        `E2E_SCREENSHOT must be one of: off, on, only-on-failure, true, false (got '${value}')`,
      );
  }
}

function readVideoMode(value: string | undefined): VideoMode {
  switch (value) {
    case undefined:
    case "":
    case "0":
    case "false":
    case "off":
      return "off";
    case "1":
    case "true":
    case "failure":
    case "retain-on-failure":
      return "retain-on-failure";
    case "first-retry":
    case "on-first-retry":
      return "on-first-retry";
    case "always":
    case "on":
      return "on";
    default:
      throw new Error(
        `E2E_VIDEO must be one of: off, on, retain-on-failure, on-first-retry, true, false (got '${value}')`,
      );
  }
}
