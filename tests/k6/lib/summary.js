function metricValue(data, metricName, field) {
  const metric = data.metrics && data.metrics[metricName];
  if (!metric || !metric.values) return undefined;
  return metric.values[field];
}

function fmtNumber(value, digits) {
  if (value === undefined || value === null || Number.isNaN(value))
    return "n/a";
  return Number(value).toFixed(digits);
}

function fmtMillis(value) {
  const formatted = fmtNumber(value, 2);
  if (formatted === "n/a") return formatted;
  return `${formatted}ms`;
}

function fmtPercent(rate) {
  if (rate === undefined || rate === null || Number.isNaN(rate)) return "n/a";
  return `${(Number(rate) * 100).toFixed(2)}%`;
}

function collectChecks(group, totals) {
  if (!group) return;

  const checks = group.checks || [];
  for (const check of checks) {
    totals.passes += check.passes || 0;
    totals.fails += check.fails || 0;
  }

  const groups = group.groups || [];
  for (const child of groups) {
    collectChecks(child, totals);
  }
}

function checkStatus(data) {
  const totals = { passes: 0, fails: 0 };
  collectChecks(data.root_group, totals);
  return `${totals.passes} passed, ${totals.fails} failed`;
}

export function scenarioSummary(scenarioName, data) {
  const checksRate = metricValue(data, "checks", "rate");
  const failedRate = metricValue(data, "http_req_failed", "rate");
  const reqCount = metricValue(data, "http_reqs", "count");
  const reqRate = metricValue(data, "http_reqs", "rate");
  const p95 = metricValue(data, "http_req_duration", "p(95)");
  const p99 = metricValue(data, "http_req_duration", "p(99)");
  const avg = metricValue(data, "http_req_duration", "avg");

  return [
    "",
    `Keydock k6 summary: ${scenarioName}`,
    "--------------------------------",
    `checks: ${fmtPercent(checksRate)} (${checkStatus(data)})`,
    `http failures: ${fmtPercent(failedRate)}`,
    `requests: ${fmtNumber(reqCount, 0)} total (${fmtNumber(reqRate, 2)}/s)`,
    `duration: avg=${fmtMillis(avg)} p95=${fmtMillis(p95)} p99=${fmtMillis(p99)}`,
    "",
  ].join("\n");
}

export function writeSummary(scenarioName, data) {
  const text = scenarioSummary(scenarioName, data);
  const outputs = {
    stdout: text,
  };

  if (__ENV.K6_SUMMARY_TEXT) {
    outputs[__ENV.K6_SUMMARY_TEXT] = text;
  }
  if (__ENV.K6_SUMMARY_JSON) {
    outputs[__ENV.K6_SUMMARY_JSON] = JSON.stringify(data, null, 2);
  }

  return outputs;
}
