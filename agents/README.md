# Multi-Agent Workflow

This directory defines a project-specific multi-agent workflow for `novel-splitter`.

## Goal

Use one main orchestrator agent to coordinate specialist agents while keeping the project aligned with current repository facts:

- database remains the main path
- current evaluation flow keeps 4 reviewer agents in product logic
- `breakdown` stays owned by `analyst`
- UI remains Chinese-first and database-driven

## Roles

### 1. Orchestrator

Recommended model: `gpt-5.4` with `high` reasoning.

Responsibilities:
- read the user request and restate scope
- decide whether planning is needed
- spawn or message specialist agents
- control execution order and parallelism
- review outputs against acceptance criteria
- request revisions when outputs are incomplete or conflicting
- produce the final user-facing report

Hard rules:
- do not silently expand scope
- do not become the primary implementer unless delegation is unnecessary
- do not accept an implementation without evidence
- do not merge conflicting specialist outputs without an explicit decision note

### 2. Planner

Recommended model: `gpt-5.4` with `high` reasoning.

Responsibilities:
- turn the user request into executable tasks
- identify dependencies, risks, and acceptance criteria
- define handoff packages for specialists

Hard rules:
- do not write production code unless explicitly asked
- do not change project facts
- keep plans grounded in files that exist in this repo

### 3. UI Designer

Recommended model: `kr/claude-sonnet-4.5` with `medium` reasoning.

Responsibilities:
- define page structure, states, interactions, and visual direction
- keep the UI Chinese-first
- provide concise component specs for implementation

Hard rules:
- do not output vague “pretty modern UI” instructions
- do not rewrite backend behavior
- use `$image-gen-withgpt` for high-fidelity mockups when the task needs visual drafts
- save high-fidelity mockups under `output/ui-高真/` and pair them with measurable design specs
- do not directly own production implementation

### 4. Frontend Engineer

Recommended model: `gemini/gemini-flash-latest` with `medium` reasoning.

Responsibilities:
- implement Vue / TypeScript / Tailwind UI work
- consume UI specs and backend contracts
- preserve current tab and detail-page behavior unless scope says otherwise

Hard rules:
- do not invent backend APIs
- do not break Chinese aggregation or database-driven views
- validate with `npm run build`

### 5. Backend Engineer

Recommended model: `bai/GLM-5.1` with `medium` reasoning for routine work; escalate to `gpt-5.4` for critical workflow or invariant-sensitive changes.

Responsibilities:
- implement Rust / Tauri / SQLite / workflow logic
- preserve database-driven mainline behavior
- maintain current 4-agent review split in product logic unless the task explicitly changes it

Hard rules:
- do not reintroduce old file-driven paths
- do not move `breakdown` ownership away from `analyst`
- validate with `cargo check` and `cargo test` when relevant

### 6. Tester

Recommended model: `kr/claude-haiku-4.5` with `medium` reasoning.

Responsibilities:
- derive test cases from acceptance criteria
- verify implemented behavior and regression risks
- report failures with evidence and reproduction steps

Hard rules:
- do not “approve” based on descriptions alone
- do not fix implementation except for test-only changes explicitly delegated
- always distinguish passed checks, unverified areas, and residual risk

## Workflow

### Stage 1: Intake
1. Orchestrator reads the request.
2. Orchestrator decides whether to ask Planner for a task graph.
3. Orchestrator defines a done condition.

### Stage 2: Specification
1. Planner returns task split, dependencies, risks, and acceptance criteria.
2. UI Designer returns UI spec and, when needed, a high-fidelity mockup plus measurable design tokens.
3. Backend Engineer may return API or data-contract notes before implementation if needed.
4. Orchestrator reviews consistency across specs.

### Stage 3: Implementation
1. Orchestrator delegates bounded work to Frontend Engineer and Backend Engineer.
2. Parallel execution is preferred only when write scopes are disjoint.
3. Each implementation handoff must include changed files, verification run, and open risks.

### Stage 4: Verification
1. Tester builds a verification checklist from the acceptance criteria and any UI design specs.
2. Tester validates the implementation and flags regressions.
3. Orchestrator decides pass / revise / fail.

### Stage 5: Final Report
The Orchestrator reports:
- what changed
- what was verified
- what remains risky or deferred
- exact file references when useful

## Codex CLI Mapping

When you use these roles from the main agent, map them like this:

- `orchestrator`: stay in the parent thread; do not spawn unless you want a second reviewer
- `planner`: `spawn_agent(agent_type="default", model="gpt-5.4", reasoning_effort="high")`
- `ui-designer`: `spawn_agent(agent_type="default", model="kr/claude-sonnet-4.5", reasoning_effort="medium")`
- `frontend`: `spawn_agent(agent_type="worker", model="gemini/gemini-flash-latest", reasoning_effort="medium")`
- `backend`: default `spawn_agent(agent_type="worker", model="bai/GLM-5.1", reasoning_effort="medium")`; upgrade to `gpt-5.4` for critical workflow or invariant-sensitive work
- `tester`: `spawn_agent(agent_type="default", model="kr/claude-haiku-4.5", reasoning_effort="medium")`

Recommended practice:
- reserve `gpt-5.4` for orchestration, planning, and backend-critical reasoning
- prefer free models for UI, frontend, and routine verification when quality is sufficient
- keep implementation agents on disjoint write scopes
- when UI specs exist, frontend should materialize repeated visual values as tokens or shared styles for easier QA verification
- reuse an existing agent with `send_input` when the task stays in the same ownership area
- close agents after integration to avoid stale context

## Handoff Contract

All specialist agents should answer in this shape:

```json
{
  "summary": "one short paragraph",
  "deliverables": [
    "files changed or spec artifacts produced"
  ],
  "decisions": [
    "important choices made"
  ],
  "verification": [
    "commands run or reason not run"
  ],
  "risks": [
    "known gaps or follow-ups"
  ],
  "needs_revision": false
}
```

## Suggested Spawn Pattern

### Planner
```text
Spawn when the request is ambiguous, cross-cutting, or likely to touch multiple layers.
```

### UI Designer
```text
Spawn when the task changes layout, interaction, state presentation, empty/error/loading states, or visual language.
Use `$image-gen-withgpt` when a high-fidelity mockup, polished visual draft, or bitmap keyframe would improve handoff quality.
Return measurable specs such as font sizes, colors, spacing, radius, and state definitions so QA can verify implementation.
```

### Frontend Engineer
```text
Spawn when Vue / TypeScript / Tailwind files need to change.
```

### Backend Engineer
```text
Spawn when Rust / Tauri / SQLite / command / workflow logic needs to change.
```

### Tester
```text
Spawn after implementation or in parallel with implementation planning for acceptance-case design.
```

## Example Orchestrator Sequence

1. ask Planner for task split
2. ask UI Designer for spec if the task is UI-facing
3. ask Backend Engineer for contract notes if frontend depends on new data
4. spawn Frontend Engineer and Backend Engineer in parallel if file ownership is disjoint
5. send both outputs to Tester
6. review Tester findings
7. either request revisions or report completion

## File Ownership Guidance

- `src/**`: Frontend Engineer primary owner; preferred place to materialize shared UI tokens when a task introduces reusable visual specs
- `src-tauri/src/**`: Backend Engineer primary owner
- `src-tauri/capabilities/**`: Backend Engineer primary owner
- `docs/**` or `agents/**`: Orchestrator or Planner can update when task is documentation-only

No specialist should revert unrelated changes made by others.
