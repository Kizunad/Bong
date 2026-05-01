---
name: consume-plan
description: Consume a Bong docs/plan-name.md end to end through an isolated worktree, implementation, tests, PR, review handling, merge, and cleanup. Use when the user invokes $consume-plan, /consume-plan, asks to consume a plan, or asks Codex to implement a Bong plan file through the repository plan workflow.
---

# Bong Plan Consumption

This skill adapts the repository command at `commands/consume-plan.md` for Codex. Treat that file as the authoritative workflow; this skill is only the Codex entrypoint and guardrail layer.

## Inputs

Accept a single plan name. Normalize these forms to `PLAN`:

- `foo` -> `docs/plan-foo.md`
- `plan-foo` -> `docs/plan-foo.md`
- `docs/plan-foo.md` -> `docs/plan-foo.md`

Reject skeleton or archived plans:

- `docs/plans-skeleton/plan-<PLAN>.md`
- `docs/finished_plans/plan-<PLAN>.md`

## Required Context

Before changing files, read:

1. `AGENTS.md`
2. `CLAUDE.md`
3. `commands/consume-plan.md`
4. `docs/plan-<PLAN>.md`

If these conflict, obey higher-priority active instructions first, then `AGENTS.md`, then `CLAUDE.md`, then `commands/consume-plan.md`, then the plan.

## Execution Rules

- Use `commands/consume-plan.md` as the detailed step-by-step procedure.
- Execute implementation in the worktree path defined by the command, never in the main checkout.
- Start every shell block by explicitly setting or using the worktree path; do not rely on persistent cwd.
- Keep changes scoped to the active plan.
- Preserve the command's failure behavior: stop with the failing step, relevant logs, PR URL if any, and worktree path; do not clean up a failed worktree.
- Do not skip failed TODOs unless the active plan workflow explicitly permits it.
- Do not bypass tests, hooks, or review checks.

## Codex Adaptation

The source command was written for slash-command systems. In Codex:

- Interpret `$ARGUMENTS` as the normalized `PLAN`.
- Use Codex tools for edits and command execution.
- Use subagents only when the user explicitly authorizes delegation, or when active session instructions permit it.
- Respect the active dangerous-operation policy. If the current session requires explicit confirmation for `git commit`, `git push`, `gh pr merge`, branch deletion, or worktree cleanup, ask before running those commands even if the source command is written as a zero-interaction flow.

## Summary Output

On success, report:

- merged PR URL
- consumed plan file
- key commits or squash result
- tests run
- cleanup status

On failure, report:

- failed step
- exact failing command
- concise error summary
- worktree path
- PR URL if one exists
- whether there are uncommitted changes
