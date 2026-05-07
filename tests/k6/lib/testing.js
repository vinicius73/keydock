import { expect as baseExpect } from "https://jslib.k6.io/k6-testing/0.6.1/index.js";

// We primarily use non-retrying (sync) assertions for protocol testing.
// Keep configuration centralized so scenarios stay consistent.
export const expect = baseExpect.configure({
  display: "pretty",
});
