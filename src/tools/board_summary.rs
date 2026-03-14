use super::traits::{Tool, ToolResult};
use crate::config::Config;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct BoardSummaryTool {
    config: Arc<Config>,
}

impl BoardSummaryTool {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for BoardSummaryTool {
    fn name(&self) -> &str {
        "board_summary"
    }

    fn description(&self) -> &str {
        "Get a summary of the task board — counts by status, overdue, and stale tasks"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        if !self.config.board.enabled {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("board is disabled by config (board.enabled=false)".to_string()),
            });
        }

        let url = format!("{}/board/summary", self.config.board.api_url);
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
