import { must, parseJson } from "./client.js";

export function assertApiError(res, expectedCode, expectedMessage, ctx) {
  must(
    res,
    { [`api error: status=${expectedCode}`]: (r) => r.status === expectedCode },
    ctx,
  );
  const body = parseJson(res, ctx);
  const err = body && body.error;
  if (!err || err.code !== expectedCode || err.message !== expectedMessage) {
    throw new Error(
      `${ctx}: expected error {code:${expectedCode},message:${expectedMessage}} got ${JSON.stringify(body)}`,
    );
  }
}

export function parseAccessToken(res, ctx) {
  const body = parseJson(res, ctx, { redactAccessToken: true });
  if (
    typeof body.access_token !== "string" ||
    body.access_token.indexOf(".") === -1
  ) {
    throw new Error(`${ctx}: missing/invalid access_token`);
  }
  return body.access_token;
}
