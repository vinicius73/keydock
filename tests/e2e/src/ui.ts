import { createApp, reactive } from "vue";

import E2eApp from "./E2eApp.vue";

export type StepState = "pending" | "running" | "done" | "error";

export type StepView = {
  id: string;
  label: string;
  value: string;
  state: StepState;
  attrs: Record<string, string>;
};

export type AppModel = {
  title: string;
  description: string;
  status: StepState;
  statusLabel: string;
  bucketId: string;
  errorName: string;
  errorStatus: string;
  errorDetail: string;
  logs: string[];
  steps: StepView[];
};

let activeModel: AppModel | undefined;

export function mountE2eApp(input: {
  title: string;
  description: string;
  steps: Array<{ id: string; label: string }>;
  bucketId?: string;
}): AppModel {
  const model = reactive<AppModel>({
    title: input.title,
    description: input.description,
    status: "pending",
    statusLabel: "idle",
    bucketId: input.bucketId ?? "not-configured",
    errorName: "none",
    errorStatus: "",
    errorDetail: "",
    logs: [],
    steps: input.steps.map((step) => ({
      id: step.id,
      label: step.label,
      value: "pending",
      state: "pending",
      attrs: {},
    })),
  });

  activeModel = model;
  createApp(E2eApp, { model }).mount("#app");
  return model;
}

export function setText(testId: string, value: string): void {
  const model = requireModel();
  switch (testId) {
    case "bucket-id":
      model.bucketId = value;
      return;
    case "error-name":
      model.errorName = value;
      return;
    case "error-status":
      model.errorStatus = value;
      return;
    case "error-detail":
      model.errorDetail = value;
      return;
    default:
      step(testId).value = value;
  }
}

export function setAttr(testId: string, name: string, value: string): void {
  step(testId).attrs[name] = value;
}

export function setStatus(state: StepState, label?: string): void {
  const model = requireModel();
  model.status = state;
  model.statusLabel = label ?? state;
}

export function setStep(testId: string, state: StepState, text?: string): void {
  const target = step(testId);
  target.state = state;
  target.value = text ?? state;
}

export function appendLog(message: string): void {
  requireModel().logs.push(message);
}

export function renderError(error: unknown): void {
  setStatus("error", "error");
  setText("error-name", errorName(error));
  setText("error-detail", errorDetail(error));
  if (typeof error === "object" && error !== null && "status" in error) {
    setText("error-status", String(error.status));
  }
}

function requireModel(): AppModel {
  if (activeModel === undefined) {
    throw new Error("E2E Vue app is not mounted");
  }
  return activeModel;
}

function step(testId: string): StepView {
  const target = requireModel().steps.find((item) => item.id === testId);
  if (target === undefined) {
    throw new Error(`missing step with data-testid="${testId}"`);
  }
  return target;
}

function errorName(error: unknown): string {
  if (error instanceof Error) {
    return error.name;
  }
  return "UnknownError";
}

function errorDetail(error: unknown): string {
  if (typeof error === "object" && error !== null && "detail" in error) {
    return String(error.detail);
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
