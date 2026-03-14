use super::traits::{Tool, ToolResult};
use crate::config::Config;
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct BoardDeleteTool {
    config: Arc<Config>,
    security: Arc<SecurityPolicy>,
}

impl BoardDeleteTool {
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
impl Tool for BoardDeleteTool {
    fn name(&self) -> &str {
        "board_delete"
    }

    fn description(&self) -> &str {
        "Delete a task from the board"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task UUID to delete" }
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

        if let Some(err) = self.enforce_mutation_allowed("board_delete") {
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

        let client = reqwest::Client::new();
        match client
            .delete(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.board.api_key),
            )
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    Ok(ToolResult {
                        success: true,
                        output: if text.is_empty() {
                            format!("Task {} deleted", task_id)
                        } else {
                            text
                        },
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
