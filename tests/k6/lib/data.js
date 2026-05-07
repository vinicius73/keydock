import { RUN_ID } from "./env.js";

let seq = 0;
const runtimeNonce = Math.random().toString(36).slice(2, 10);

function nextSeq() {
  seq += 1;
  return seq;
}

function suffix() {
  const vu = typeof __VU === "undefined" ? "setup" : `vu${__VU}`;
  const iter = typeof __ITER === "undefined" ? "setup" : `it${__ITER}`;
  return `${RUN_ID}-${vu}-${iter}-${runtimeNonce}-${nextSeq()}`;
}

export function uniqueEmail() {
  return `k6-${suffix()}@example.com`;
}

export function uniqueKey(prefix) {
  return `${prefix}-${suffix()}`;
}

export function bucketSetupRestrictedAndSigned() {
  const s = suffix();
  return {
    email: `k6-${s}@example.com`,
    read_key: `r-${s}`,
    write_key: `w-${s}`,
    secret_key: `sec-${s}`,
    signing_key: `sign-${s}`,
  };
}
