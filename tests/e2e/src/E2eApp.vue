<script setup lang="ts">
import type { AppModel, StepState, StepView } from "./ui";

defineProps<{
  model: AppModel;
}>();

const badgeClassByState: Record<StepState, string> = {
  pending: "border-slate-700 bg-slate-900 text-slate-400",
  running: "border-amber-400/40 bg-amber-400/10 text-amber-200",
  done: "border-emerald-400/40 bg-emerald-400/10 text-emerald-200",
  error: "border-rose-400/40 bg-rose-400/10 text-rose-200",
};

function badgeClass(state: StepState): string {
  return `inline-flex rounded-full border px-3 py-1 text-xs font-semibold uppercase tracking-[0.25em] ${badgeClassByState[state]}`;
}

function cardClass(step: StepView): string {
  return `rounded-xl border p-4 ${badgeClassByState[step.state]}`;
}
</script>

<template>
  <main class="mx-auto flex min-h-screen max-w-4xl flex-col gap-8 px-6 py-10">
    <header class="space-y-3">
      <p class="text-sm font-semibold tracking-[0.35em] text-cyan-300 uppercase">Keydock SDK E2E</p>
      <div class="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h1 class="text-4xl font-semibold tracking-tight">
            {{ model.title }}
          </h1>
          <p class="mt-2 max-w-2xl text-slate-400">{{ model.description }}</p>
        </div>
        <span data-testid="app-status" :data-state="model.status" :class="badgeClass(model.status)">
          {{ model.statusLabel }}
        </span>
      </div>
    </header>

    <section class="grid gap-4 sm:grid-cols-2">
      <article
        v-for="step in model.steps"
        :key="step.id"
        :data-state="step.state"
        :class="cardClass(step)"
      >
        <p class="text-xs font-semibold tracking-[0.25em] uppercase opacity-70">
          {{ step.label }}
        </p>
        <p
          :data-testid="step.id"
          :data-state="step.state"
          class="mt-2 font-mono text-sm break-words"
          v-bind="step.attrs"
        >
          {{ step.value }}
        </p>
      </article>
    </section>

    <section class="rounded-2xl border border-slate-800 bg-slate-900/60 p-5">
      <div class="grid gap-3 font-mono text-sm text-slate-300">
        <p>
          Bucket:
          <span data-testid="bucket-id" class="text-cyan-200">{{ model.bucketId }}</span>
        </p>
        <p>
          Error:
          <span data-testid="error-name">{{ model.errorName }}</span>
          <span data-testid="error-status">{{ model.errorStatus }}</span>
          <span data-testid="error-detail">{{ model.errorDetail }}</span>
        </p>
      </div>
      <ol data-testid="event-log" class="mt-5 space-y-3 text-sm">
        <li
          v-for="(message, index) in model.logs"
          :key="index"
          class="border-l border-cyan-300/30 pl-3 text-slate-300"
        >
          {{ message }}
        </li>
      </ol>
    </section>
  </main>
</template>
