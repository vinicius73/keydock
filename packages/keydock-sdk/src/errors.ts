type ErrorInput = {
  message: string;
  cause?: unknown;
};

function errorOptions(cause: unknown): ErrorOptions | undefined {
  return cause === undefined ? undefined : { cause };
}

export class KeydockError extends Error {
  readonly name = "KeydockError";
  readonly status: number;
  readonly code: number;
  readonly detail: string;
  readonly response?: Response;
  readonly request?: Request;

  constructor(input: {
    status: number;
    code: number;
    detail: string;
    response?: Response;
    request?: Request;
    cause?: unknown;
  }) {
    super(`Keydock request failed: ${input.detail}`, errorOptions(input.cause));
    this.status = input.status;
    this.code = input.code;
    this.detail = input.detail;
    if (input.response !== undefined) {
      this.response = input.response;
    }
    if (input.request !== undefined) {
      this.request = input.request;
    }
  }
}

export class KeydockNetworkError extends Error {
  readonly name = "KeydockNetworkError";

  constructor(input: ErrorInput) {
    super(input.message, errorOptions(input.cause));
  }
}

export class KeydockTimeoutError extends Error {
  readonly name = "KeydockTimeoutError";

  constructor(input: ErrorInput) {
    super(input.message, errorOptions(input.cause));
  }
}

export class KeydockValidationError extends TypeError {
  readonly name = "KeydockValidationError";

  constructor(message: string, options?: { cause?: unknown }) {
    super(message, errorOptions(options?.cause));
  }
}
