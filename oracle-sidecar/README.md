# oracle-sidecar

Implementation scaffold for the Oracle-OS sidecar runtime.

## Implemented in this milestone
- JSON-RPC 2.0 request handling over stdio (line-delimited JSON)
- `health.ping`
- `task.start`
- `task.status`
- `task.cancel`
- `task.complete`
- `task.list`
- In-memory task registry + persisted task store at `.oracle/runs/tasks.json`
- Unit tests for task lifecycle, list behavior, persistence, and error handling

## Quick run
```bash
cargo run
```
Then send JSON-RPC lines through stdin, for example:
```json
{"jsonrpc":"2.0","id":1,"method":"health.ping","params":{}}
```

Create a task:
```json
{"jsonrpc":"2.0","id":2,"method":"task.start","params":{"prompt":"build dashboard","mode":"ARCHITECT"}}
```

List tasks:
```json
{"jsonrpc":"2.0","id":3,"method":"task.list","params":{}}
```

## Notes
This is still a foundation milestone. Future work should add:
- event streaming (`task.progress`, `verification.report`, etc.)
- LSP/Playwright tool adapters
- shadow-workspace merge and policy enforcement
