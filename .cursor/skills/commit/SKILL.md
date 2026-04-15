---
name: commit
description: Generate Git Commit Message (Gitmoji Convention)
disable-model-invocation: true
---

# Generate Git Commit Message (Gitmoji Convention)

## Goal

Generate one commit message for staged changes using Gitmoji + Conventional Commits.

## Inputs

You MUST run these read-only git commands and use their outputs:

- `git rev-parse --abbrev-ref HEAD`
- `git diff --staged`
- `git status --porcelain=v1`

## Output contract (exact)

Return exactly two sections:

1. `### Commit message`

- One fenced code block with ONLY the commit message.

2. `### Next step`

- One sentence asking whether to run the commit now (no command).

If `git diff --staged` output is empty:

- In `### Commit message`, output a code block with exactly: `NO_STAGED_CHANGES`
- In `### Next step`, ask to stage files and retry.

If you cannot execute git commands or do not have their outputs:

- In `### Commit message`, output a code block with exactly: `NEEDS_GIT_OUTPUT`
- In `### Next step`, ask the user to paste outputs for the three commands above.

## Build rules

### 1) Pick primary intent

Pick the dominant change; do not combine types in the title.

### 2) Optional `(context)` from branch

First match wins:

- If branch contains `ABC-123`, use `(ABC-123)`.
- Else if branch contains `feature/<scope>` / `fix/<scope>` / `chore/<scope>`, use `(<scope>)`.
- Else omit `(context)` (do not invent).

Normalize `<scope>`:

- kebab-case
- remove prefixes like `feature/`, `bugfix/`, `hotfix/`

### 3) Title (single line)

`<emoji> <type>: (context) <imperative summary>`

- `(context)` optional; if present, exactly one pair of parentheses.
- Imperative, concise, no trailing period.
- Avoid vague words ("update", "changes") when possible.

### 4) Body

After one blank line, include 2–6 bullets:

- Start each bullet with `- `.
- Prefer outcomes/behavior and key areas/files.
- Mention tests and any notable risk/migration.

## Constraints

- You MUST run the three read-only git commands listed above before deciding `NO_STAGED_CHANGES`.
- Do NOT run any mutating git commands (`git add`, `git commit`, `git push`, `git checkout`, `git reset`, `git rebase`).
- Ask only the single confirmation question.

## Gitmoji examples

- ✨ feat: add a new feature
- 🐛 fix: fix a bug
- 🚑 hotfix: urgent critical fix
- ♻️ refactor: restructure code without changing behavior
- 🚀 perf: improve performance
- 🎨 style: code style changes (formatting, spacing, naming)
- ✏️ typofix: fix typos
- 🧹 chore: maintenance tasks (build, configs, housekeeping)
- ✅ test: add or update tests
- 📸 snapshot: update test snapshots
- 🧪 mock: add or update mocks
- 📝 docs: update documentation
- 🔊 logs: update log messages
- ➕ add: add a dependency
- ➖ remove: remove a dependency
- 🔥 remove: remove code or files
- ⬆️ upgrade: upgrade dependencies
- ⬇️ downgrade: downgrade dependencies
- ⌛ deps: lock or pin dependencies
- 📦 package: package or publish
- 🔖 release: new version or tag
- ⏪ revert: revert changes
- 🔒 security: fix security issues
- 🛂 auth: update authentication/authorization
- 🔧 config: update configuration
- 💚 ci: update CI pipelines
- 👷 build: update build system
- 🛠️ tool: update dev tools
- 🗃️ db: database changes
- 🗂️ assets: add or update assets (images, icons, fonts)
- 🏗️ infrastructure: infrastructure changes
- 🐳 docker: Docker-related changes
- 🆕 init: initialize project
- 🚧 wip: work in progress
- 🗑️ deprecate: mark code or files as deprecated
- 🔀 merge: merge branches
- 📱 mobile: mobile/responsive changes
- 💄 ui: UI/UX improvements
- 🌐 i18n: internationalization/localization
- 🧑‍💻 dev: developer experience improvements
