use super::traits::{Tool, ToolResult};
use crate::config::Config;
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

pub struct BoardCreateTool {
    config: Arc<Config>,
    security: Arc<SecurityPolicy>,
}

impl BoardCreateTool {
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
impl Tool for BoardCreateTool {
    fn name(&self) -> &str {
        "board_create"
    }

    fn description(&self) -> &str {
        "Create a new task on the board"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Task title" },
                "description": { "type": "string", "description": "Task description" },
                "priority": { "type": "integer", "description": "Priority: 0 (low), 1 (normal), 2 (urgent)", "enum": [0, 1, 2] },
                "category": { "type": "string", "description": "Category label (e.g. briefing, wellness, meals, admin)" },
                "source": { "type": "string", "description": "Source: daily_plan, employer, agent, manual", "enum": ["daily_plan", "employer", "agent", "manual"] },
                "recurrence_key": { "type": "string", "description": "Dedup key for recurring tasks (e.g. 'morning_briefing'). Server rejects if a pending/in_progress task with the same key exists." },
                "due_date": { "type": "string", "description": "Due date in YYYY-MM-DD format" },
                "parent_task_id": { "type": "string", "description": "Parent task UUID for sub-tasks" }
            },
            "required": ["title"],
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

        if let Some(err) = self.enforce_mutation_allowed("board_create") {
            return Ok(err);
        }

        let url = format!("{}/board/tasks", self.config.board.api_url);

        let mut body = serde_json::Map::new();
        if let Some(title) = args.get("title") {
            body.insert("title".to_string(), title.clone());
        }
        if let Some(description) = args.get("description") {
            body.insert("description".to_string(), description.clone());
        }
        if let Some(priority) = args.get("priority") {
            body.insert("priority".to_string(), priority.clone());
        }
        if let Some(category) = args.get("category") {
            body.insert("category".to_string(), category.clone());
        }
        if let Some(source) = args.get("source") {
            body.insert("source".to_string(), source.clone());
        }
        if let Some(recurrence_key) = args.get("recurrence_key") {
            body.insert("recurrence_key".to_string(), recurrence_key.clone());
        }
        if let Some(due_date) = args.get("due_date") {
            body.insert("due_date".to_string(), due_date.clone());
        }
        if let Some(parent_task_id) = args.get("parent_task_id") {
            body.insert("parent_task_id".to_string(), parent_task_id.clone());
        }

        let client = reqwest::Client::new();
        match client
            .post(&url)
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
