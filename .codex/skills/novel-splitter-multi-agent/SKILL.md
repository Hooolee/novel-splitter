---
name: "novel-splitter-multi-agent"
description: "Project-specific multi-agent development workflow for novel-splitter. Use when Codex should act as an orchestrator that dispatches planner, UI designer, frontend, backend, and tester agents for work in this repo; especially for cross-functional feature work, UI spec plus high-fidelity mockup tasks, backend workflow changes, or any task that benefits from structured planning, bounded ownership, QA review, and final orchestration."
---

# Novel Splitter Multi-Agent Workflow

Read `PROJECT.md` first. Respect `AGENTS.md` throughout the task.

## Purpose

Use the main thread as the orchestrator. Dispatch bounded specialist work, review evidence, request revisions when needed, and report final results to the user.

## Source of Truth

Load only the files needed for the task:
- `agents/README.md` for the overall workflow
- `agents/orchestration-playbook.md` for dispatch order and review gates
- `agents/task-packets/ui-task.json` for UI-heavy tasks
- `agents/task-packets/backend-task.json` for backend-critical tasks
- `agents/task-packets/fullstack-task.json` for full-stack tasks
- `agents/*.md` and `agents/*.toml` only for the specialists you actually use

## Project Invariants

Preserve these repo facts during orchestration and review:
- database-driven flow is the main path
- current product evaluation flow keeps 4 reviewer agents: `reader`, `editor`, `author`, `analyst`
- `breakdown` stays owned by `analyst`
- UI stays Chinese-first and database-driven
- UI high-fidelity mockups go under `output/ui-高真/`
- UI mockups must be paired with measurable specs: font sizes, weights, colors, spacing, radius, states, and acceptance checks

## Default Runtime Mapping

Use these models unless the task clearly needs a different tradeoff:
- `orchestrator`: parent thread, review-heavy, use `gpt-5.4`
- `planner`: `spawn_agent(... model="gpt-5.4", reasoning_effort="high")`
- `ui-designer`: `spawn_agent(... model="kr/claude-sonnet-4.5", reasoning_effort="medium")`
- `frontend`: `spawn_agent(... agent_type="worker", model="gemini/gemini-flash-latest", reasoning_effort="medium")`
- `backend`: default `spawn_agent(... agent_type="worker", model="bai/GLM-5.1", reasoning_effort="medium")`
- `tester`: `spawn_agent(... model="kr/claude-haiku-4.5", reasoning_effort="medium")`

Upgrade backend to `gpt-5.4` when the task touches any of these:
- database main-path behavior
- aggregation or report invariants
- `breakdown` ownership
- cross-command workflow behavior
- risky Rust refactors that are hard to validate cheaply

## Orchestration Procedure

1. Read `PROJECT.md` and restate scope.
2. Choose one task packet template from `agents/task-packets/`.
3. If the task is ambiguous or cross-cutting, dispatch Planner first.
4. If the task is UI-facing, dispatch UI Designer before Frontend.
5. If the task changes backend contracts, dispatch Backend before Frontend final implementation.
6. Keep write scopes disjoint when running agents in parallel.
7. Require every specialist to return summary, changed files or artifacts, decisions, verification, and risks.
8. Send implementation outputs to Tester before final completion.
9. Reject outputs that lack evidence, violate repo invariants, or omit measurable UI specs.
10. Only the orchestrator gives the final user-facing report.

## UI-Specific Rule

When a task needs a polished visual draft, instruct UI Designer to use `$image-gen-withgpt` and save images under `output/ui-高真/`. Treat the image as a design artifact, not production implementation.

## Verification Rule

Prefer this verification order when code changes are involved:
1. `cargo check`
2. `cargo test`
3. `npm run build`
4. task-specific checks if the request requires them

When reporting completion with a commit, include evidence in the same message per `AGENTS.md`.

## Deliverable Standard

The orchestrator should expect specialist outputs to cover:
- summary
- changed files or generated artifacts
- important decisions
- verification commands or reasons not run
- risks or follow-ups
- whether revision is still needed
