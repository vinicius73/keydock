import { withResponseContext } from "./client.js";

export function tags(scenario, flow) {
  // Keep tags consistent across helpers so k6 time series stay low-cardinality and comparable.
  return { scenario, flow };
}

export function checkRes(res, ctx, fn, opts) {
  // Wrap assertions so a failure includes response context (status + truncated body),
  // which makes contract regressions much faster to diagnose.
  withResponseContext(res, ctx, opts, () => fn(res));
  return res;
}
