# Orchestration Playbook

This playbook shows how the main agent should run a real task in `novel-splitter` using the role files in `agents/`.

## Objective

Use one orchestrator thread to:
- interpret the user request
- dispatch bounded work to specialist agents
- review evidence from each agent
- request revisions when needed
- report a final outcome to the user

## Default Agent Mapping

- `orchestrator`: parent thread, default reviewer and dispatcher
- `planner`: `agents/planner.toml`
- `ui-designer`: `agents/ui-designer.toml`
- `frontend`: `agents/frontend.toml`
- `backend`: `agents/backend.toml`
- `tester`: `agents/tester.toml`

## Dispatch Rules

### Use Planner when
- the request touches multiple layers
- the request is ambiguous
- acceptance criteria are not obvious
- you expect frontend and backend to coordinate

### Use UI Designer when
- the task changes layout, hierarchy, interaction, or visual language
- the task needs a high-fidelity mockup under `output/ui-高真/`
- frontend implementation would benefit from measurable design specs

### Use Frontend when
- `src/**` changes are required
- accepted UI specs already exist or the change is clearly frontend-only

### Use Backend when
- `src-tauri/src/**` or `src-tauri/capabilities/**` changes are required
- command behavior, database flow, workflow logic, or aggregation logic changes
- preserve database-driven mainline and product evaluation invariants

### Use Tester when
- implementation is complete enough to validate
- you need an acceptance report before user-facing completion
- the task has UI work, behavior changes, or regression risk

## Review Gates

The orchestrator should reject or revise an agent output when:
- scope drift appears
- repo facts are violated
- changed files are missing
- verification evidence is missing
- UI work lacks measurable specs
- QA findings are not addressed

## Standard Task Packet

Use a packet like this whenever you delegate:

```json
{
  "goal": "what to achieve",
  "scope": [
    "in-scope items"
  ],
  "constraints": [
    "repo rules and task-specific rules"
  ],
  "write_scope": [
    "files or directories this agent may edit"
  ],
  "inputs": [
    "relevant files, specs, or prior agent outputs"
  ],
  "deliverables": [
    "what the agent must return"
  ],
  "verification": [
    "commands or checks expected"
  ],
  "done_criteria": [
    "observable completion criteria"
  ]
}
```

## Example Flow A: UI-Heavy Feature

User request example:
> 优化书库详情页，让信息层级更清楚，并补一版高真稿。

### Step 1: Orchestrator intake
- restate the request
- note that this is UI-facing and likely frontend-only unless data shape changes
- define completion as: approved UI spec, implementation, verification, final report

### Step 2: Planner packet
Send Planner a packet asking for:
- task split
- risks
- acceptance criteria
- whether backend changes are necessary

Expected result:
- likely no backend change unless new data is needed
- clear UI acceptance checklist

### Step 3: UI Designer packet
Send UI Designer a packet with:
- target screen
- current constraints: Chinese-first UI, existing detail-page flow, database-backed data
- output requirement: spec + mockup under `output/ui-高真/` + measurable design tokens + acceptance checklist

Expected result:
- one or more mockups
- component hierarchy
- design tokens
- QA-friendly checks

### Step 4: Frontend packet
Send Frontend a packet with:
- approved UI designer output
- write scope: `src/**`
- instruction to implement recurring values as shared tokens or styles
- requirement to run `npm run build`

Expected result:
- changed Vue/Tailwind files
- tokenized visual values where practical
- build evidence

### Step 5: Tester packet
Send Tester a packet with:
- planner acceptance criteria
- UI designer spec refs
- frontend changed files
- instruction to use the UI review report template

Expected result:
- structured pass/revise/fail report
- exact findings with expected vs actual behavior

### Step 6: Orchestrator review
- if QA passes: produce user report
- if QA revises: send exact findings back to Frontend or UI Designer as needed

## Example Flow B: Backend-Critical Workflow Fix

User request example:
> 修复扫描后报告聚合逻辑，确保 breakdown 仍然只由 analyst 输出。

### Step 1: Orchestrator intake
- classify as backend-critical
- explicitly preserve database-driven mainline and current 4-agent evaluation split

### Step 2: Planner packet
Request:
- likely affected files
- invariants that must not break
- acceptance criteria for aggregation behavior

### Step 3: Backend packet
Send Backend a packet with:
- write scope: `src-tauri/src/**`, `src-tauri/capabilities/**`
- invariants:
  - do not reintroduce file-driven flow
  - keep `breakdown` owned by `analyst`
  - preserve 4-agent evaluation structure
- verification: `cargo check`, `cargo test`, plus targeted reasoning notes if behavior is not easily executable

### Step 4: Tester packet
Send Tester:
- planner criteria
- backend diff summary
- instruction to verify invariants and any available command/test evidence

### Step 5: Orchestrator review
- reject completion if invariant evidence is weak
- only close after QA report is acceptable

## Example Flow C: Full-Stack Change

User request example:
> 在拆书雷达里增加一个新的报告筛选条件，并展示在报告列表中。

Suggested dispatch order:
1. Planner
2. Backend for data contract and command changes
3. UI Designer if new control or state presentation needs spec work
4. Frontend after data contract is clear
5. Tester after implementation

## Revision Loop

When returning a task for revision, the orchestrator should send:
- the finding or mismatch
- the expected behavior or spec reference
- the exact files or areas to revisit
- whether the agent may edit or should only respond with a patch plan

Example revision note:
```text
Revise the detail header implementation.
Expected title size is 24px with weight 600 from the approved UI spec.
Actual implementation uses a smaller visual hierarchy.
Please update only `src/components/...` related files and rerun `npm run build`.
```

## Completion Checklist for Orchestrator

Before reporting completion to the user, confirm:
- the right specialists were used
- outputs stayed within scope
- changed files are listed
- verification evidence exists
- UI work includes measurable specs when applicable
- tester recommendation is pass, or any remaining risk is clearly disclosed

## Minimal First Run Recommendation

For your first real trial in this repo, prefer a task with:
- one UI screen or one command-level backend fix
- clear acceptance criteria
- no large refactor

This keeps the orchestration loop easy to inspect before scaling up.
