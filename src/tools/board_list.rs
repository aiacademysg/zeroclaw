use super::traits::{Tool, ToolResult};
use crate::config::Config;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct BoardListTool {
    config: Arc<Config>,
}

impl BoardListTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for BoardListTool {
    fn name(&self) -> &str {
        "board_list"
    }

    fn description(&self) -> &str {
        "List tasks on the board, optionally filtered by status"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status: pending, in_progress, done, archived",
                    "enum": ["pending", "in_progress", "done", "archived"]
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of tasks to return (default 50, max 200)"
                }
            },
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

        let mut url = format!("{}/board/tasks", self.config.board.api_url);
        let mut params = Vec::new();

        if let Some(status) = args.get("status").and_then(|v| v.as_str()) {
            params.push(format!("status={}", status));
        }
        if let Some(limit) = args.get("limit").and_then(|v| v.as_u64()) {
            params.push(format!("limit={}", limit));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let client = reqwest::Client::new();
        match client
            .get(&url)
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
