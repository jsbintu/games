# Oracle-OS Product Specification

## Vision
Oracle-OS is an autonomous engineering system that extends beyond one-shot generation.

### Core Differentiator
- **Versus Kombai**: Kombai is primarily one-way design-to-code. Oracle-OS is full lifecycle engineering: **Design → Code → Verify → Debug → Fix**.
- **Versus Copilot**: Copilot predicts from text patterns; Oracle-OS verifies behavior using **Language Server Protocol (LSP)** feedback and **computed DOM styles** from a headless browser.

## 1) System Architecture — Hybrid Sidecar
A split architecture avoids editor freezes by moving heavy tasks out of the extension host.

| Component | Technology | Responsibility |
| --- | --- | --- |
| Frontend | VS Code Webview (React) | Flight Recorder UI, chat interface, ghost text decorations |
| Bridge | JSON-RPC | Connects frontend and backend |
| Engine | Rust sidecar binary | File I/O, LSP client, vector database (LanceDB) |
| Eye | Playwright (headless) | Renders hidden browser view for screenshots and computed styles |
| Safety | Shadow workspace (`.oracle/shadow`) | Writes changes in isolation; merges to real `src/` only after green checks |

## 2) Intelligence Layer — Router-Based Multi-Agent System
Oracle-OS uses a routing layer to select specialist behavior.

### 2.1 Router (Gatekeeper)
**Purpose**: Analyze user intent and dispatch to the right specialist.

```json
{
  "role": "system",
  "content": "You are the ORACLE ROUTER. Analyze the user request and route to the correct specialist.\n\nROUTING LOGIC:\n1. IF request requires new files, structural changes, or UI creation -> ROUTE: [ARCHITECT]\n2. IF request involves fixing bugs, black screens, or console errors -> ROUTE: [DETECTIVE]\n3. IF request involves scripts, installs, or bulk refactors -> ROUTE: [MULTI_TASKER]\n\nOUTPUT FORMAT: JSON { \"agent\": \"ARCHITECT\", \"reasoning\": \"User wants a new dashboard layout.\" }"
}
```

### 2.2 Specialist A — Architect (UI Manifestation)
**Goal**: Zero-dissonance creation with atomic design and token discipline.

#### Laws of Creation
1. **Scope First**: Emit `file_structure_tree` JSON before code.
2. **Token Strictness**: Never hardcode raw colors when tokens exist.
3. **Atomic Decomposition**: Split components that exceed 150 lines.
4. **LSP Verification**: Verify symbols and imports via `lsp_query`.

#### Output Protocol
1. Plan (ASCII tree)
2. Scaffolding (file creation)
3. Implementation (content injection)

### 2.3 Specialist B — Detective (System Debugger)
**Goal**: Root-cause elimination with proof-driven debugging.

#### Laws of Investigation
1. **The Eye**: For visual bugs, inspect computed styles (`visual_inspect`) instead of trusting source.
2. **The Trace**: Add temporary `[DEBUG:ID]` logs at every data transition.
3. **The Sandbox**: Isolate failures in `test-routes/debug-view`.
4. **Cleanup**: Remove all temporary logs and routes after verification.

#### Debug Loop
**Log → Isolate → Fix → Verify → Cleanup**

## 3) Tooling Infrastructure ("The Scepter")
These tools form the system's high-assurance control surface.

### 3.1 The Eye — Visual Verification
```json
{
  "name": "visual_inspect",
  "description": "Render the current local URL and inspect the computed DOM state.",
  "parameters": {
    "url": "http://localhost:3000/dashboard",
    "selector": "#submit-btn",
    "return_type": "COMPUTED_STYLES"
  }
}
```

### 3.2 The Nervous System — LSP Query
```json
{
  "name": "lsp_query",
  "description": "Query the TypeScript/Rust Language Server for symbol details.",
  "parameters": {
    "symbol": "UserCardProps",
    "action": "GET_DEFINITION"
  }
}
```

### 3.3 The Hands — Surgical Editing
```json
{
  "name": "surgical_edit",
  "description": "Precise search and replace operation.",
  "parameters": {
    "filepath": "src/App.tsx",
    "search_block": "previous code snippet...",
    "replace_block": "new code snippet..."
  }
}
```

## 4) Knowledge Engine — Dynamic RAG (Grimoire Injection)
Dynamic, runtime context injection based on project signals.

### Injection Protocol
1. **Scanner**: Rust sidecar watches `package.json` and config files.
2. **Injector**: Adds relevant skill cards to system prompts.

### Example Skill Cards
| Trigger File | Injected Skill | Law |
| --- | --- | --- |
| `tailwind.config.ts` | Codex of Styling | Prefer `@layer` utilities and tokenized classes over arbitrary values |
| `tauri.conf.json` | Bridge Protocol | Wrap `invoke()` calls in try/catch and use typed payload interfaces |
| `vite.config.ts` | Bundler Wisdom | Use `import.meta.env`; serve static assets from `/public` |

## 5) Self-Healing Automation Loops

### 5.1 Iron-Clad Loop (Creation)
1. Generate code into `.oracle/shadow`.
2. Run lint and type checks on shadow output.
3. Feed diagnostics back to the agent while status is “Verifying…”.
4. Merge to real source only when checks pass.

### 5.2 Visual Truth Loop (Debugging)
1. Capture computed DOM state via `visual_inspect`.
2. Compare rendered values to design tokens.
3. Diagnose CSS collisions (e.g., `flex-shrink`, layout constraints).
4. Patch and re-verify until visual contract is met.

## 6) Implementation Roadmap

### Phase 1 (Weeks 1–4): Foundation
- Build Rust sidecar with tree-sitter parsing and LanceDB indexing.
- Implement shadow workspace file mirroring and guarded merge.

### Phase 2 (Weeks 5–8): Senses
- Integrate `tsserver`/Rust analyzer for `lsp_query`.
- Add Playwright runtime and computed-style extraction for `visual_inspect`.

### Phase 3 (Weeks 9–12): Brain
- Implement router + multi-agent dispatch over LLM API.
- Build package/config scanner with prompt-time skill injection.

### Phase 4 (Weeks 13+): Polish
- Ship Flight Recorder UI showing Planning → Coding → Verifying transitions.
- Add ghost-text inline suggestions driven by specialist outputs.

## 7) Acceptance Criteria (MVP)
- Agent can route at least 90% of sampled tasks to the correct specialist.
- Shadow workspace prevents direct writes to production source during generation.
- Lint + type checks gate all merges from shadow workspace.
- Visual debugger can report computed styles for selected elements.
- Debug flow supports evidence-based fix verification and cleanup.
