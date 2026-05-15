use async_trait::async_trait;
use crate::provider::ToolDefinition;
use crate::tools::{Tool, ToolOutput};
use std::sync::{Arc, Mutex};

pub struct CompactTool {
    pub triggered: Arc<Mutex<bool>>,
}

impl CompactTool {
    pub fn new() -> Self {
        Self {
            triggered: Arc::new(Mutex::new(false)),
        }
    }
}

#[async_trait]
impl Tool for CompactTool {
    fn name(&self) -> &str {
        "compact_conversation"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compact_conversation".into(),
            description: "Trigger conversation compaction to reduce context length.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn execute(&self, _args: serde_json::Value) -> ToolOutput {
        let mut flag = self.triggered.lock().unwrap();
        *flag = true;
        ToolOutput {
            content: "Compaction triggered".into(),
            is_error: false,
        }
    }
}
