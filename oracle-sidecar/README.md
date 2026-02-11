# oracle-sidecar

Initial implementation scaffold for the Oracle-OS sidecar runtime.

## Implemented in this milestone
- JSON-RPC 2.0 request handling over stdio (line-delimited JSON)
- `health.ping`
- `task.start`
- `task.status`
- `task.cancel`
- In-memory task registry
- Unit tests for the basic task lifecycle and error handling

## Quick run
```bash
cargo run
```
Then send JSON-RPC lines through stdin, for example:
```json
{"jsonrpc":"2.0","id":1,"method":"health.ping","params":{}}
```

## Notes
This is a foundation milestone and not the complete product runtime yet.
Future milestones should add:
- event streaming (`task.progress`, `verification.report`, etc.)
- persistent run store under `.oracle/runs`
- LSP/Playwright tool adapters
- shadow-workspace merge and policy enforcement
