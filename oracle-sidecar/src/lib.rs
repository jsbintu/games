use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
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

#[derive(Debug, Serialize, Deserialize, Default)]
struct TaskStore {
    tasks: Vec<TaskRecord>,
}

pub struct Sidecar {
    tasks: HashMap<Uuid, TaskRecord>,
    run_dir: PathBuf,
}

impl Default for Sidecar {
    fn default() -> Self {
        Self::new(".oracle/runs")
    }
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
    #[error("internal error: {0}")]
    Internal(String),
}

impl RpcError {
    fn code(&self) -> i32 {
        match self {
            RpcError::Parse(_) => -32700,
            RpcError::MethodNotFound => -32601,
            RpcError::InvalidParams(_) => -32602,
            RpcError::TaskNotFound => -32004,
            RpcError::Internal(_) => -32603,
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
    pub fn new(run_dir: impl AsRef<Path>) -> Self {
        let run_dir = run_dir.as_ref().to_path_buf();
        let mut sidecar = Self {
            tasks: HashMap::new(),
            run_dir,
        };
        let _ = sidecar.load_store();
        sidecar
    }

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
                    "task.complete" => self.task_complete(&request.params).map(|rec| json!(rec)),
                    "task.list" => Ok(json!(self.task_list())),
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
        self.persist_store()?;
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
        self.update_task_status(params, TaskStatus::Cancelled)
    }

    fn task_complete(&mut self, params: &Value) -> Result<TaskRecord, RpcError> {
        self.update_task_status(params, TaskStatus::Completed)
    }

    fn update_task_status(
        &mut self,
        params: &Value,
        new_status: TaskStatus,
    ) -> Result<TaskRecord, RpcError> {
        #[derive(Deserialize)]
        struct TaskIdParams {
            task_id: Uuid,
        }

        let parsed: TaskIdParams = serde_json::from_value(params.clone())
            .map_err(|e| RpcError::InvalidParams(e.to_string()))?;

        let task = self
            .tasks
            .get_mut(&parsed.task_id)
            .ok_or(RpcError::TaskNotFound)?;

        task.status = new_status;
        let updated = task.clone();
        self.persist_store()?;
        Ok(updated)
    }

    fn task_list(&self) -> Vec<TaskRecord> {
        let mut tasks: Vec<_> = self.tasks.values().cloned().collect();
        tasks.sort_by_key(|task| task.task_id);
        tasks
    }

    fn store_path(&self) -> PathBuf {
        self.run_dir.join("tasks.json")
    }

    fn load_store(&mut self) -> Result<(), RpcError> {
        let path = self.store_path();
        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| RpcError::Internal(format!("failed to read {}: {}", path.display(), e)))?;
        let store: TaskStore = serde_json::from_str(&content).map_err(|e| {
            RpcError::Internal(format!("failed to parse {}: {}", path.display(), e))
        })?;

        self.tasks = store
            .tasks
            .into_iter()
            .map(|task| (task.task_id, task))
            .collect();

        Ok(())
    }

    fn persist_store(&self) -> Result<(), RpcError> {
        fs::create_dir_all(&self.run_dir).map_err(|e| {
            RpcError::Internal(format!(
                "failed to create run dir {}: {}",
                self.run_dir.display(),
                e
            ))
        })?;

        let store = TaskStore {
            tasks: self.task_list(),
        };

        let content = serde_json::to_string_pretty(&store)
            .map_err(|e| RpcError::Internal(format!("failed to serialize task store: {}", e)))?;

        let path = self.store_path();
        fs::write(&path, content)
            .map_err(|e| RpcError::Internal(format!("failed to write {}: {}", path.display(), e)))
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

    fn unique_test_dir() -> PathBuf {
        std::env::temp_dir().join(format!("oracle-sidecar-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn starts_and_reads_status() {
        let mut sidecar = Sidecar::new(unique_test_dir());
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
    fn cancels_and_completes_task() {
        let mut sidecar = Sidecar::new(unique_test_dir());
        let start = sidecar.handle_json_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"task.start","params":{"prompt":"fix ui"}}"#,
        );
        let task_id = start["result"]["task_id"].as_str().unwrap();

        let cancel = sidecar.handle_json_line(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"task.cancel\",\"params\":{{\"task_id\":\"{}\"}}}}",
            task_id
        ));

        assert_eq!(cancel["result"]["status"], "cancelled");

        let complete = sidecar.handle_json_line(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"task.complete\",\"params\":{{\"task_id\":\"{}\"}}}}",
            task_id
        ));

        assert_eq!(complete["result"]["status"], "completed");
    }

    #[test]
    fn lists_tasks() {
        let mut sidecar = Sidecar::new(unique_test_dir());
        sidecar.handle_json_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"task.start","params":{"prompt":"first"}}"#,
        );
        sidecar.handle_json_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"task.start","params":{"prompt":"second"}}"#,
        );

        let list = sidecar
            .handle_json_line(r#"{"jsonrpc":"2.0","id":3,"method":"task.list","params":{}}"#);

        assert_eq!(list["result"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn persists_store_across_instances() {
        let run_dir = unique_test_dir();
        let mut first = Sidecar::new(&run_dir);
        let start = first.handle_json_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"task.start","params":{"prompt":"persist me"}}"#,
        );

        let task_id = start["result"]["task_id"].as_str().unwrap().to_string();

        let second = Sidecar::new(&run_dir);
        let loaded = second
            .tasks
            .get(&Uuid::parse_str(&task_id).unwrap())
            .unwrap();

        assert_eq!(loaded.prompt, "persist me");
        assert_eq!(loaded.status, TaskStatus::Running);
    }

    #[test]
    fn returns_method_not_found() {
        let mut sidecar = Sidecar::new(unique_test_dir());
        let response = sidecar
            .handle_json_line(r#"{"jsonrpc":"2.0","id":1,"method":"unknown.method","params":{}}"#);

        assert_eq!(response["error"]["code"], -32601);
    }
}
