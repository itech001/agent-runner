use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

pub struct TodosTool {
    pub state: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodosTool {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Tool for TodosTool {
    fn name(&self) -> &str {
        "write_todos"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_todos".into(),
            description: "Update the internal todo list.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "content": { "type": "string" },
                                "status": { "type": "string" },
                                "priority": { "type": "string" }
                            },
                            "required": ["id", "content", "status", "priority"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> ToolOutput {
        let todos: Vec<TodoItem> = match serde_json::from_value(args["todos"].clone()) {
            Ok(t) => t,
            Err(e) => {
                return ToolOutput {
                    content: format!("Invalid todos format: {}", e),
                    is_error: true,
                }
            }
        };
        let count = todos.len();
        let mut state = self.state.lock().unwrap();
        *state = todos;
        ToolOutput {
            content: format!("Updated {} todos", count),
            is_error: false,
        }
    }
}
