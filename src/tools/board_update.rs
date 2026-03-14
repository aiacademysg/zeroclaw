use super::traits::{Tool, ToolResult};
use crate::config::Config;
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct BoardUpdateTool {
    config: Arc<Config>,
    security: Arc<SecurityPolicy>,
}

impl BoardUpdateTool {
    pub fn new(config: Arc<Config>, security: Arc<SecurityPolicy>) -> Self {
        Self { config, security }
    }

    fn enforce_mutation_allowed(&self, action: &str) -> Option<ToolResult> {
        if !self.security.can_act() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Security policy: read-only mode, cannot perform '{action}'"
                )),
            });
        }

        if self.security.is_rate_limited() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: too many actions in the last hour".to_string()),
            });
        }

        if !self.security.record_action() {
            return Some(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Rate limit exceeded: action budget exhausted".to_string()),
            });
        }

        None
    }
}

#[async_trait]
impl Tool for BoardUpdateTool {
    fn name(&self) -> &str {
        "board_update"
    }

    fn description(&self) -> &str {
        "Update a task on the board — change status, priority, or add a result summary"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task UUID to update" },
                "title": { "type": "string" },
                "description": { "type": "string" },
                "status": { "type": "string", "description": "New status: pending, in_progress, done, archived", "enum": ["pending", "in_progress", "done", "archived"] },
                "priority": { "type": "integer", "enum": [0, 1, 2] },
                "category": { "type": "string" },
                "due_date": { "type": "string" },
                "result_summary": { "type": "string", "description": "Summary of what was accomplished (set when marking done)" }
            },
            "required": ["task_id"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if !self.config.board.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("board is disabled by config (board.enabled=false)".to_string()),
            });
        }

        if let Some(err) = self.enforce_mutation_allowed("board_update") {
            return Ok(err);
        }

        let task_id = match args.get("task_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required 'task_id' parameter".to_string()),
                });
            }
        };

        let url = format!("{}/board/tasks/{}", self.config.board.api_url, task_id);

        let mut body = serde_json::Map::new();
        if let Some(title) = args.get("title") {
            body.insert("title".to_string(), title.clone());
        }
        if let Some(description) = args.get("description") {
            body.insert("description".to_string(), description.clone());
        }
        if let Some(status) = args.get("status") {
            body.insert("status".to_string(), status.clone());
        }
        if let Some(priority) = args.get("priority") {
            body.insert("priority".to_string(), priority.clone());
        }
        if let Some(category) = args.get("category") {
            body.insert("category".to_string(), category.clone());
        }
        if let Some(due_date) = args.get("due_date") {
            body.insert("due_date".to_string(), due_date.clone());
        }
        if let Some(result_summary) = args.get("result_summary") {
            body.insert("result_summary".to_string(), result_summary.clone());
        }

        let client = reqwest::Client::new();
        match client
            .put(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.board.api_key),
            )
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    Ok(ToolResult {
                        success: true,
                        output: text,
                        error: None,
                    })
                } else {
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Board API error {}: {}", status, text)),
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Board API request failed: {}", e)),
            }),
        }
    }
}
