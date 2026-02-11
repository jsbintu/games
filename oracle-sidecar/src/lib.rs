use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Cancelled,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRecord {
    pub task_id: Uuid,
    pub prompt: String,
    pub mode: String,
    pub status: TaskStatus,
}

#[derive(Default)]
pub struct Sidecar {
    tasks: HashMap<Uuid, TaskRecord>,
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("task not found")]
    TaskNotFound,
    #[error("method not found")]
    MethodNotFound,
    #[error("parse error: {0}")]
    Parse(String),
}

impl RpcError {
    fn code(&self) -> i32 {
        match self {
            RpcError::Parse(_) => -32700,
            RpcError::MethodNotFound => -32601,
            RpcError::InvalidParams(_) => -32602,
            RpcError::TaskNotFound => -32004,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

impl Sidecar {
    pub fn handle_json_line(&mut self, line: &str) -> Value {
        let req: Result<RpcRequest, _> = serde_json::from_str(line);
        match req {
            Ok(request) => {
                if request.jsonrpc != "2.0" {
                    return error_response(
                        request.id,
                        RpcError::InvalidParams("jsonrpc must be 2.0".into()),
                    );
                }

                let response = match request.method.as_str() {
                    "health.ping" => Ok(json!({ "ok": true })),
                    "task.start" => self.task_start(&request.params).map(|rec| json!(rec)),
                    "task.status" => self.task_status(&request.params).map(|rec| json!(rec)),
                    "task.cancel" => self.task_cancel(&request.params).map(|rec| json!(rec)),
                    _ => Err(RpcError::MethodNotFound),
                };

                match response {
                    Ok(result) => success_response(request.id, result),
                    Err(err) => error_response(request.id, err),
                }
            }
            Err(err) => error_response(None, RpcError::Parse(err.to_string())),
        }
    }

    fn task_start(&mut self, params: &Value) -> Result<TaskRecord, RpcError> {
        #[derive(Deserialize)]
        struct StartParams {
            prompt: String,
            #[serde(default = "default_mode")]
            mode: String,
        }

        let parsed: StartParams = serde_json::from_value(params.clone())
            .map_err(|e| RpcError::InvalidParams(e.to_string()))?;

        let record = TaskRecord {
            task_id: Uuid::new_v4(),
            prompt: parsed.prompt,
            mode: parsed.mode,
            status: TaskStatus::Running,
        };

        self.tasks.insert(record.task_id, record.clone());
        Ok(record)
    }

    fn task_status(&self, params: &Value) -> Result<TaskRecord, RpcError> {
        #[derive(Deserialize)]
        struct StatusParams {
            task_id: Uuid,
        }

        let parsed: StatusParams = serde_json::from_value(params.clone())
            .map_err(|e| RpcError::InvalidParams(e.to_string()))?;

        self.tasks
            .get(&parsed.task_id)
            .cloned()
            .ok_or(RpcError::TaskNotFound)
    }

    fn task_cancel(&mut self, params: &Value) -> Result<TaskRecord, RpcError> {
        #[derive(Deserialize)]
        struct CancelParams {
            task_id: Uuid,
        }

        let parsed: CancelParams = serde_json::from_value(params.clone())
            .map_err(|e| RpcError::InvalidParams(e.to_string()))?;

        let task = self
            .tasks
            .get_mut(&parsed.task_id)
            .ok_or(RpcError::TaskNotFound)?;

        task.status = TaskStatus::Cancelled;
        Ok(task.clone())
    }

    pub fn run_stdio<R: BufRead, W: Write>(
        &mut self,
        reader: R,
        mut writer: W,
    ) -> std::io::Result<()> {
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let response = self.handle_json_line(&line);
            writeln!(writer, "{}", response)?;
            writer.flush()?;
        }
        Ok(())
    }
}

fn default_mode() -> String {
    "ARCHITECT".to_string()
}

fn success_response(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn error_response(id: Option<Value>, err: RpcError) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": err.code(),
            "message": err.to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn starts_and_reads_status() {
        let mut sidecar = Sidecar::default();
        let response = sidecar.handle_json_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"task.start","params":{"prompt":"build dashboard","mode":"ARCHITECT"}}"#,
        );

        let task_id = response["result"]["task_id"].as_str().unwrap().to_string();
        let status = sidecar.handle_json_line(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"task.status\",\"params\":{{\"task_id\":\"{}\"}}}}",
            task_id
        ));

        assert_eq!(status["result"]["status"], "running");
    }

    #[test]
    fn cancels_task() {
        let mut sidecar = Sidecar::default();
        let start = sidecar.handle_json_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"task.start","params":{"prompt":"fix ui"}}"#,
        );
        let task_id = start["result"]["task_id"].as_str().unwrap();

        let cancel = sidecar.handle_json_line(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"task.cancel\",\"params\":{{\"task_id\":\"{}\"}}}}",
            task_id
        ));

        assert_eq!(cancel["result"]["status"], "cancelled");
    }

    #[test]
    fn returns_method_not_found() {
        let mut sidecar = Sidecar::default();
        let response = sidecar
            .handle_json_line(r#"{"jsonrpc":"2.0","id":1,"method":"unknown.method","params":{}}"#);

        assert_eq!(response["error"]["code"], -32601);
    }
}
