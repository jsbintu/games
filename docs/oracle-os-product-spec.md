# Oracle-OS: Product Spec + Full Implementation Blueprint

## 0) Executive Summary
Oracle-OS is an **autonomous software engineer** that performs complete lifecycle engineering:

> **Design → Code → Verify → Debug → Fix**

Unlike single-pass code generation tools, Oracle-OS enforces correctness through:
- static truth (**LSP + diagnostics**),
- runtime truth (**tests + build checks**), and
- visual truth (**computed DOM inspection via headless browser**).

This document is intentionally implementation-oriented. It defines the system architecture, protocols, contracts, data models, workflows, and acceptance tests required to build Oracle-OS end-to-end.

---

## 1) Product Differentiation

### 1.1 Versus Design-to-Code tools
Design-to-code systems usually terminate after code synthesis.
Oracle-OS continues until software quality is proven:

1. Generate plan and code.
2. Validate with lints/types/tests.
3. Reproduce defects.
4. Isolate root cause.
5. Apply and verify fix.
6. Clean up temporary debug instrumentation.

### 1.2 Versus autocomplete copilots
Autocomplete predicts likely tokens from context.
Oracle-OS uses a **verification loop** driven by observable evidence:
- LSP symbol resolution and diagnostics,
- process exit codes for build/test,
- computed styles and dimensions from a real browser render.

---

## 2) System Architecture — Hybrid Sidecar

A split architecture keeps editor UX responsive while supporting heavy operations.

| Layer | Technology | Responsibility |
| --- | --- | --- |
| Frontend | VS Code Webview (React) | Flight Recorder timeline, chat panel, tool execution trace, ghost text |
| Bridge | JSON-RPC 2.0 | Request/response + streaming events between webview and sidecar |
| Engine | Rust sidecar binary | File operations, orchestration, tool runtime, task graph, safety policies |
| Sensors | Playwright + LSP clients | Visual and semantic verification |
| Retrieval | LanceDB (vectors) + symbol index | Context retrieval and repository memory |
| Safety | `.oracle/shadow` workspace | Isolated writes and merge-on-green behavior |

### 2.1 Process Model
- VS Code extension host launches sidecar process.
- Sidecar exposes JSON-RPC over stdio.
- Frontend receives streamed status updates and step-level logs.
- Long operations are cancellable via `task.cancel`.

### 2.2 Failure Isolation
- Sidecar crashes do not crash webview UI.
- Browser workers are pooled and time-bounded.
- LSP clients run in dedicated task contexts with restart semantics.

---

## 3) Core Runtime: Router + Specialists

### 3.1 Router Contract
Router decides which specialist to invoke.

**Input**
```json
{
  "request": "fix black screen on settings page",
  "workspace_state": {
    "changed_files": ["src/settings/page.tsx"],
    "recent_errors": ["TypeError: Cannot read ..."]
  }
}
```

**Output**
```json
{
  "agent": "DETECTIVE",
  "reasoning": "User reports runtime visual failure and error symptoms."
}
```

### 3.2 Specialist Definitions

#### ARCHITECT (Creation)
Primary when tasks require new files, structure, or UI.

Must do:
1. Emit file tree plan.
2. Validate imports/symbol existence with `lsp_query`.
3. Respect design tokens (no hardcoded style constants when token exists).
4. Split oversized components (>150 LOC target) into composable units.

#### DETECTIVE (Debugging)
Primary for defects, regressions, and visual/runtime inconsistencies.

Must do:
1. Add scoped debug probes (`[DEBUG:<id>]`) across data flow boundaries.
2. Isolate to debug route/sandbox.
3. Use visual inspector for CSS/layout assertions.
4. Remove probes/routes after fix verification.

#### MULTI_TASKER (Bulk operations)
Primary for mass edits, scripts, migrations, installs.

Must do:
1. Batch plans with deterministic ordering.
2. Run incremental checks to avoid high-cost global failures late.
3. Provide reversible checkpoint strategy.

---

## 4) Tooling Contracts (The Scepter)

## 4.1 `visual_inspect`
Render a URL and return runtime DOM truth.

```json
{
  "name": "visual_inspect",
  "parameters": {
    "url": "http://localhost:3000/dashboard",
    "selector": "#submit-btn",
    "return_type": "COMPUTED_STYLES"
  }
}
```

**Response shape**
```json
{
  "exists": true,
  "box": { "x": 120, "y": 412, "width": 96, "height": 40 },
  "computed": {
    "display": "inline-flex",
    "position": "relative",
    "color": "rgb(255, 255, 255)",
    "backgroundColor": "rgb(22, 119, 255)",
    "fontSize": "14px"
  },
  "screenshot_path": ".oracle/artifacts/inspect-submit-btn.png"
}
```

## 4.2 `lsp_query`
Query symbol definitions, references, hover, diagnostics.

```json
{
  "name": "lsp_query",
  "parameters": {
    "language": "typescript",
    "action": "GET_DEFINITION",
    "symbol": "UserCardProps"
  }
}
```

**Response shape**
```json
{
  "found": true,
  "location": "src/components/user-card.tsx:8:18",
  "signature": "interface UserCardProps { name: string; age: number }"
}
```

## 4.3 `surgical_edit`
Precise block replacement with preconditions.

```json
{
  "name": "surgical_edit",
  "parameters": {
    "filepath": "src/App.tsx",
    "search_block": "const title = 'Old';",
    "replace_block": "const title = 'New';"
  }
}
```

**Safety rules**
- Must fail if `search_block` appears zero times.
- Must fail if appears more than once unless `occurrence_index` is provided.
- Must emit post-edit diff hunk.

---

## 5) Dynamic RAG: Grimoire Injection

### 5.1 Scanner Inputs
- `package.json`
- `tailwind.config.*`
- `vite.config.*`
- `tauri.conf.json`
- CI manifests and lint config

### 5.2 Injection Output
Prompt receives only relevant skill cards (minimal context).

| Trigger | Injected Grimoire | Enforcement |
| --- | --- | --- |
| Tailwind config present | Codex of Styling | Prefer tokenized utilities over arbitrary values |
| Tauri config present | Bridge Protocol | Type-safe `invoke`, robust error handling |
| Vite config present | Bundler Wisdom | `import.meta.env`, static assets in `/public` |

### 5.3 Retrieval Ranking
Hybrid retrieval score:

`score = 0.55 * semantic_similarity + 0.30 * symbol_overlap + 0.15 * recency_weight`

---

## 6) Safety Model: Shadow Workspace

### 6.1 Layout
```text
.oracle/
  shadow/                 # mirrored writable workspace
  patches/                # generated patch sets
  artifacts/              # screenshots, logs, traces
  runs/<run_id>/          # task-level metadata and event stream
```

### 6.2 Write/Merge Policy
1. Agent writes only into `.oracle/shadow`.
2. Verification suite runs in shadow context.
3. If checks pass, sidecar applies minimal patch to real workspace.
4. If checks fail, merge is blocked and feedback loops to specialist.

### 6.3 Merge Preconditions
- lint pass
- typecheck pass
- targeted tests pass
- no unresolved conflict markers
- no temporary debug probes remaining (unless user asked to keep)

---

## 7) Verification Loops

### 7.1 Iron-Clad Loop (Creation)
1. **Plan**: specialist outputs scope + file tree.
2. **Generate**: create/edit in shadow.
3. **Check**: run lint/typecheck/tests.
4. **Repair**: auto-fix failures with constrained retries.
5. **Merge**: apply patch only on full green.

### 7.2 Visual Truth Loop (Debugging)
1. Capture computed style for failing element(s).
2. Compare with expected token contract.
3. Trace style source chain (class, inline, inherited, cascade order).
4. Patch root cause (layout collision, missing class, specificity).
5. Re-capture and assert expected values.
6. Remove temporary debug-only code.

### 7.3 Retry Strategy
- Max retries per phase: 3
- Exponential delay: 0s, 1s, 3s
- Abort conditions: repeated identical error signature, infra timeout, policy violation

---

## 8) JSON-RPC API Specification

### 8.1 Methods
- `task.start`
- `task.cancel`
- `task.status`
- `tool.invoke`
- `workspace.diff`
- `workspace.merge`

### 8.2 Example `task.start`
```json
{
  "jsonrpc": "2.0",
  "id": 12,
  "method": "task.start",
  "params": {
    "prompt": "Build settings dashboard",
    "mode": "ARCHITECT"
  }
}
```

### 8.3 Event Stream Types
- `task.progress`
- `task.log`
- `tool.started`
- `tool.result`
- `verification.report`
- `task.completed`
- `task.failed`

---

## 9) Data Model

### 9.1 Run Record
```json
{
  "run_id": "run_2026_02_11_001",
  "agent": "DETECTIVE",
  "status": "failed",
  "started_at": "2026-02-11T10:00:00Z",
  "ended_at": "2026-02-11T10:02:14Z",
  "files_touched": ["src/app/settings.tsx"],
  "checks": {
    "lint": "pass",
    "typecheck": "pass",
    "tests": "fail"
  }
}
```

### 9.2 Vector Chunks (LanceDB)
Fields:
- `chunk_id`
- `path`
- `language`
- `content`
- `embedding`
- `symbols[]`
- `updated_at`

---

## 10) Frontend UX (Flight Recorder)

### 10.1 Timeline Stages
1. Routing
2. Planning
3. Coding
4. Verifying
5. Debugging (if needed)
6. Merging

### 10.2 UI Guarantees
- Every tool call shown with start/end timestamps.
- Every code edit attached to file diff preview.
- Every failed check accompanied by actionable error excerpt.
- User can cancel any running task.

### 10.3 Ghost Text
- Inline suggestions sourced from Architect output.
- Suggestions blocked if conflicting diagnostics exist at insertion site.

---

## 11) Implementation Roadmap

### Phase 1 (Weeks 1–4): Foundation
- Rust sidecar process + JSON-RPC transport.
- Shadow workspace mirror + patch application engine.
- Baseline run/event persistence.

### Phase 2 (Weeks 5–8): Senses
- LSP integration for TypeScript + Rust.
- Playwright integration for `visual_inspect`.
- Artifact persistence and preview wiring.

### Phase 3 (Weeks 9–12): Brain
- Router + specialist orchestration engine.
- Dynamic RAG scanner and selective prompt injection.
- Retry and policy framework.

### Phase 4 (Weeks 13+): Polish
- Flight Recorder UX refinements.
- Ghost text ergonomics and confidence gating.
- Performance hardening and telemetry dashboards.

---

## 12) Security, Privacy, and Guardrails
- No silent network exfiltration of source files.
- Redaction pipeline for secrets in logs and artifacts.
- Per-tool allowlist and scope-limited file operations.
- Mandatory user-visible diff before merge in interactive mode.

---

## 13) Definition of Done (MVP)
MVP is considered complete when all criteria are met:

1. **Routing quality**: ≥90% correct specialist selection on benchmark set.
2. **Safety**: zero direct writes to production workspace during generation.
3. **Verification**: merge blocked unless lint/typecheck/tests pass.
4. **Visual debugging**: computed style + screenshot returned for selected element.
5. **Cleanup integrity**: temporary debug routes/log probes removed after successful fix.
6. **Observability**: every run emits complete event timeline.

---

## 14) First Engineering Backlog (Concrete)

1. Implement sidecar bootstrap (`oracle-sidecar`) with stdio JSON-RPC.
2. Create shadow sync engine with path mapping and patch output.
3. Integrate TypeScript LSP query adapter.
4. Implement Playwright worker pool and `visual_inspect`.
5. Build verification runner (lint/typecheck/test orchestration).
6. Add router and specialist execution graph.
7. Persist run metadata + artifacts under `.oracle/runs`.
8. Implement frontend Flight Recorder panel with event subscriptions.
9. Add merge gate policies and failure explanations.
10. Ship benchmark harness for routing + verification correctness.

This backlog is the minimal ordered path to a usable Oracle-OS alpha.
