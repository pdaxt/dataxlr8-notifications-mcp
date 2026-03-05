use dataxlr8_mcp_core::mcp::{error_result, get_bool, get_i64, get_str, json_result, make_schema};
use dataxlr8_mcp_core::Database;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ServerHandler;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

// ============================================================================
// Constants
// ============================================================================

const VALID_CHANNELS: &[&str] = &["email", "slack", "in_app"];
const VALID_PRIORITIES: &[&str] = &["low", "normal", "high", "urgent"];
const VALID_EVENT_TYPES: &[&str] = &["deal_moved", "task_overdue", "score_threshold"];
const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;
const DEFAULT_OFFSET: i64 = 0;

// ============================================================================
// Validation helpers
// ============================================================================

/// Extract a required string param, trimmed. Returns Err(CallToolResult) on missing/empty.
fn require_str_trimmed(args: &serde_json::Value, key: &str) -> Result<String, CallToolResult> {
    match get_str(args, key) {
        Some(v) => {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                Err(error_result(&format!(
                    "Parameter '{key}' must not be empty"
                )))
            } else {
                Ok(trimmed)
            }
        }
        None => Err(error_result(&format!(
            "Missing required parameter: {key}"
        ))),
    }
}

/// Extract an optional string param, trimmed. Returns None for missing, Err for empty-after-trim.
fn optional_str_trimmed(
    args: &serde_json::Value,
    key: &str,
) -> Result<Option<String>, CallToolResult> {
    match get_str(args, key) {
        Some(v) => {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                Err(error_result(&format!(
                    "Parameter '{key}' must not be empty when provided"
                )))
            } else {
                Ok(Some(trimmed))
            }
        }
        None => Ok(None),
    }
}

/// Validate that a string value is in an allowed set.
fn validate_enum(value: &str, allowed: &[&str], param_name: &str) -> Result<(), CallToolResult> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(error_result(&format!(
            "Invalid {param_name}: '{value}'. Must be one of: {}",
            allowed.join(", ")
        )))
    }
}

/// Extract and clamp limit/offset from args.
fn pagination(args: &serde_json::Value) -> (i64, i64) {
    let limit = get_i64(args, "limit")
        .unwrap_or(DEFAULT_LIMIT)
        .max(1)
        .min(MAX_LIMIT);
    let offset = get_i64(args, "offset")
        .unwrap_or(DEFAULT_OFFSET)
        .max(0);
    (limit, offset)
}

// ============================================================================
// Data types
// ============================================================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: String,
    pub channel: String,
    pub recipient: String,
    pub title: String,
    pub body: String,
    pub priority: String,
    pub read: bool,
    pub delivered: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Rule {
    pub id: String,
    pub event_type: String,
    pub condition: serde_json::Value,
    pub channel: String,
    pub template: String,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct NotificationStats {
    pub total_unread: i64,
    pub unread_email: i64,
    pub unread_slack: i64,
    pub unread_in_app: i64,
    pub total_delivered: i64,
    pub total_undelivered: i64,
    pub delivery_rate_pct: f64,
}

// ============================================================================
// Tool definitions
// ============================================================================

fn build_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "send_notification".into(),
            title: None,
            description: Some(
                "Create a notification for a user/channel (email, slack, in_app) with priority"
                    .into(),
            ),
            input_schema: make_schema(
                serde_json::json!({
                    "channel": { "type": "string", "enum": ["email", "slack", "in_app"], "description": "Delivery channel" },
                    "recipient": { "type": "string", "description": "Recipient identifier (email, slack ID, or user ID)" },
                    "title": { "type": "string", "description": "Notification title" },
                    "body": { "type": "string", "description": "Notification body" },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"], "description": "Priority level (default: normal)" }
                }),
                vec!["channel", "recipient", "title", "body"],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "list_notifications".into(),
            title: None,
            description: Some(
                "List notifications with optional filters by recipient, channel, priority, read status"
                    .into(),
            ),
            input_schema: make_schema(
                serde_json::json!({
                    "recipient": { "type": "string", "description": "Filter by recipient" },
                    "channel": { "type": "string", "enum": ["email", "slack", "in_app"], "description": "Filter by channel" },
                    "priority": { "type": "string", "enum": ["low", "normal", "high", "urgent"], "description": "Filter by priority" },
                    "read": { "type": "boolean", "description": "Filter by read status" },
                    "limit": { "type": "integer", "description": "Max results (default 50, max 500)" },
                    "offset": { "type": "integer", "description": "Offset for pagination (default 0)" }
                }),
                vec![],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "mark_read".into(),
            title: None,
            description: Some("Mark a notification as read".into()),
            input_schema: make_schema(
                serde_json::json!({
                    "id": { "type": "string", "description": "Notification ID" }
                }),
                vec!["id"],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "mark_all_read".into(),
            title: None,
            description: Some("Mark all notifications for a user as read".into()),
            input_schema: make_schema(
                serde_json::json!({
                    "recipient": { "type": "string", "description": "Recipient whose notifications to mark read" }
                }),
                vec!["recipient"],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "create_rule".into(),
            title: None,
            description: Some(
                "Create an auto-notification rule triggered by events (deal_moved, task_overdue, score_threshold)"
                    .into(),
            ),
            input_schema: make_schema(
                serde_json::json!({
                    "event_type": { "type": "string", "enum": ["deal_moved", "task_overdue", "score_threshold"], "description": "Event that triggers this rule" },
                    "condition": { "type": "object", "description": "JSON condition for when to fire (e.g. {\"stage\": \"closed_won\"} or {\"threshold\": 80})" },
                    "channel": { "type": "string", "enum": ["email", "slack", "in_app"], "description": "Channel to send notification on" },
                    "template": { "type": "string", "description": "Notification template text (supports {{event}} placeholders)" }
                }),
                vec!["event_type", "channel", "template"],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "list_rules".into(),
            title: None,
            description: Some("List notification rules, optionally filtered by event type or active status".into()),
            input_schema: make_schema(
                serde_json::json!({
                    "event_type": { "type": "string", "enum": ["deal_moved", "task_overdue", "score_threshold"], "description": "Filter by event type" },
                    "active": { "type": "boolean", "description": "Filter by active status" },
                    "limit": { "type": "integer", "description": "Max results (default 50, max 500)" },
                    "offset": { "type": "integer", "description": "Offset for pagination (default 0)" }
                }),
                vec![],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "delete_rule".into(),
            title: None,
            description: Some("Delete a notification rule".into()),
            input_schema: make_schema(
                serde_json::json!({
                    "id": { "type": "string", "description": "Rule ID to delete" }
                }),
                vec!["id"],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        Tool {
            name: "notification_stats".into(),
            title: None,
            description: Some(
                "Get notification stats: unread counts by channel, delivery rates".into(),
            ),
            input_schema: make_schema(
                serde_json::json!({
                    "recipient": { "type": "string", "description": "Optional: stats for a specific recipient only" }
                }),
                vec![],
            ),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
    ]
}

// ============================================================================
// MCP Server
// ============================================================================

#[derive(Clone)]
pub struct NotificationsMcpServer {
    db: Database,
}

impl NotificationsMcpServer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    // ---- Tool handlers ----

    async fn handle_send_notification(&self, args: &serde_json::Value) -> CallToolResult {
        let channel = match require_str_trimmed(args, "channel") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Err(e) = validate_enum(&channel, VALID_CHANNELS, "channel") {
            return e;
        }

        let recipient = match require_str_trimmed(args, "recipient") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let title = match require_str_trimmed(args, "title") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let body = match require_str_trimmed(args, "body") {
            Ok(v) => v,
            Err(e) => return e,
        };

        let priority = match optional_str_trimmed(args, "priority") {
            Ok(Some(v)) => {
                if let Err(e) = validate_enum(&v, VALID_PRIORITIES, "priority") {
                    return e;
                }
                v
            }
            Ok(None) => "normal".to_string(),
            Err(e) => return e,
        };

        let id = uuid::Uuid::new_v4().to_string();

        match sqlx::query_as::<_, Message>(
            "INSERT INTO notifications.messages (id, channel, recipient, title, body, priority) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
        )
        .bind(&id)
        .bind(&channel)
        .bind(&recipient)
        .bind(&title)
        .bind(&body)
        .bind(&priority)
        .fetch_one(self.db.pool())
        .await
        {
            Ok(msg) => {
                info!(id = %id, channel = %channel, recipient = %recipient, "Sent notification");
                json_result(&msg)
            }
            Err(e) => {
                error!(error = %e, channel = %channel, recipient = %recipient, "Failed to send notification");
                error_result(&format!("Failed to send notification: {e}"))
            }
        }
    }

    async fn handle_list_notifications(&self, args: &serde_json::Value) -> CallToolResult {
        let recipient = match optional_str_trimmed(args, "recipient") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let channel = match optional_str_trimmed(args, "channel") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Some(ref c) = channel {
            if let Err(e) = validate_enum(c, VALID_CHANNELS, "channel") {
                return e;
            }
        }
        let priority = match optional_str_trimmed(args, "priority") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Some(ref p) = priority {
            if let Err(e) = validate_enum(p, VALID_PRIORITIES, "priority") {
                return e;
            }
        }
        let read = get_bool(args, "read");
        let (limit, offset) = pagination(args);

        let mut sql = String::from(
            "SELECT id, channel, recipient, title, body, priority, read, delivered, created_at \
             FROM notifications.messages WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut str_params: Vec<String> = Vec::new();
        let mut bool_param: Option<bool> = None;

        if let Some(ref r) = recipient {
            sql.push_str(&format!(" AND recipient = ${param_idx}"));
            param_idx += 1;
            str_params.push(r.clone());
        }
        if let Some(ref c) = channel {
            sql.push_str(&format!(" AND channel = ${param_idx}"));
            param_idx += 1;
            str_params.push(c.clone());
        }
        if let Some(ref p) = priority {
            sql.push_str(&format!(" AND priority = ${param_idx}"));
            param_idx += 1;
            str_params.push(p.clone());
        }
        if let Some(r) = read {
            sql.push_str(&format!(" AND read = ${param_idx}"));
            param_idx += 1;
            bool_param = Some(r);
        }

        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut query = sqlx::query_as::<_, Message>(&sql);
        for p in &str_params {
            query = query.bind(p);
        }
        if let Some(r) = bool_param {
            query = query.bind(r);
        }
        query = query.bind(limit);
        query = query.bind(offset);

        match query.fetch_all(self.db.pool()).await {
            Ok(messages) => json_result(&messages),
            Err(e) => {
                error!(error = %e, "Failed to list notifications");
                error_result(&format!("Database error: {e}"))
            }
        }
    }

    async fn handle_mark_read(&self, id: &str) -> CallToolResult {
        match sqlx::query_as::<_, Message>(
            "UPDATE notifications.messages SET read = true WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        {
            Ok(Some(msg)) => {
                info!(id = %id, "Marked notification as read");
                json_result(&msg)
            }
            Ok(None) => error_result(&format!("Notification '{id}' not found")),
            Err(e) => {
                error!(error = %e, id = %id, "Failed to mark notification as read");
                error_result(&format!("Database error: {e}"))
            }
        }
    }

    async fn handle_mark_all_read(&self, recipient: &str) -> CallToolResult {
        match sqlx::query(
            "UPDATE notifications.messages SET read = true WHERE recipient = $1 AND read = false",
        )
        .bind(recipient)
        .execute(self.db.pool())
        .await
        {
            Ok(r) => {
                let count = r.rows_affected();
                info!(recipient = %recipient, count = count, "Marked all notifications as read");
                json_result(&serde_json::json!({
                    "recipient": recipient,
                    "marked_read": count
                }))
            }
            Err(e) => {
                error!(error = %e, recipient = %recipient, "Failed to mark all notifications as read");
                error_result(&format!("Database error: {e}"))
            }
        }
    }

    async fn handle_create_rule(&self, args: &serde_json::Value) -> CallToolResult {
        let event_type = match require_str_trimmed(args, "event_type") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Err(e) = validate_enum(&event_type, VALID_EVENT_TYPES, "event_type") {
            return e;
        }

        let channel = match require_str_trimmed(args, "channel") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Err(e) = validate_enum(&channel, VALID_CHANNELS, "channel") {
            return e;
        }

        let template = match require_str_trimmed(args, "template") {
            Ok(v) => v,
            Err(e) => return e,
        };

        let condition = args
            .get("condition")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let id = uuid::Uuid::new_v4().to_string();

        match sqlx::query_as::<_, Rule>(
            "INSERT INTO notifications.rules (id, event_type, condition, channel, template) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(&id)
        .bind(&event_type)
        .bind(&condition)
        .bind(&channel)
        .bind(&template)
        .fetch_one(self.db.pool())
        .await
        {
            Ok(rule) => {
                info!(id = %id, event_type = %event_type, "Created notification rule");
                json_result(&rule)
            }
            Err(e) => {
                error!(error = %e, event_type = %event_type, "Failed to create notification rule");
                error_result(&format!("Failed to create rule: {e}"))
            }
        }
    }

    async fn handle_list_rules(&self, args: &serde_json::Value) -> CallToolResult {
        let event_type = match optional_str_trimmed(args, "event_type") {
            Ok(v) => v,
            Err(e) => return e,
        };
        if let Some(ref et) = event_type {
            if let Err(e) = validate_enum(et, VALID_EVENT_TYPES, "event_type") {
                return e;
            }
        }
        let active = get_bool(args, "active");
        let (limit, offset) = pagination(args);

        let mut sql = String::from(
            "SELECT id, event_type, condition, channel, template, active, created_at \
             FROM notifications.rules WHERE 1=1",
        );
        let mut param_idx = 1u32;
        let mut str_params: Vec<String> = Vec::new();
        let mut bool_param: Option<bool> = None;

        if let Some(ref et) = event_type {
            sql.push_str(&format!(" AND event_type = ${param_idx}"));
            param_idx += 1;
            str_params.push(et.clone());
        }
        if let Some(a) = active {
            sql.push_str(&format!(" AND active = ${param_idx}"));
            param_idx += 1;
            bool_param = Some(a);
        }

        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut query = sqlx::query_as::<_, Rule>(&sql);
        for p in &str_params {
            query = query.bind(p);
        }
        if let Some(a) = bool_param {
            query = query.bind(a);
        }
        query = query.bind(limit);
        query = query.bind(offset);

        match query.fetch_all(self.db.pool()).await {
            Ok(rules) => json_result(&rules),
            Err(e) => {
                error!(error = %e, "Failed to list rules");
                error_result(&format!("Database error: {e}"))
            }
        }
    }

    async fn handle_delete_rule(&self, id: &str) -> CallToolResult {
        match sqlx::query("DELETE FROM notifications.rules WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
        {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    info!(id = %id, "Deleted notification rule");
                    json_result(&serde_json::json!({ "deleted": true, "id": id }))
                } else {
                    error_result(&format!("Rule '{id}' not found"))
                }
            }
            Err(e) => {
                error!(error = %e, id = %id, "Failed to delete rule");
                error_result(&format!("Failed to delete rule: {e}"))
            }
        }
    }

    async fn handle_notification_stats(&self, args: &serde_json::Value) -> CallToolResult {
        let recipient = match optional_str_trimmed(args, "recipient") {
            Ok(v) => v,
            Err(e) => return e,
        };

        let where_clause = if recipient.is_some() {
            " WHERE recipient = $1"
        } else {
            ""
        };
        let unread_clause = if recipient.is_some() {
            " WHERE recipient = $1 AND read = false"
        } else {
            " WHERE read = false"
        };

        // Total unread
        let total_unread: i64 = {
            let sql = format!("SELECT COUNT(*) FROM notifications.messages{unread_clause}");
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch total_unread stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        // Unread by channel
        let unread_email: i64 = {
            let sql = format!(
                "SELECT COUNT(*) FROM notifications.messages{unread_clause} AND channel = 'email'"
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch unread_email stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        let unread_slack: i64 = {
            let sql = format!(
                "SELECT COUNT(*) FROM notifications.messages{unread_clause} AND channel = 'slack'"
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch unread_slack stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        let unread_in_app: i64 = {
            let sql = format!(
                "SELECT COUNT(*) FROM notifications.messages{unread_clause} AND channel = 'in_app'"
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch unread_in_app stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        // Delivery rates
        let total_delivered: i64 = {
            let sql = format!(
                "SELECT COUNT(*) FROM notifications.messages{} AND delivered = true",
                if recipient.is_some() {
                    " WHERE recipient = $1"
                } else {
                    " WHERE 1=1"
                }
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch total_delivered stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        let total_count: i64 = {
            let sql = format!("SELECT COUNT(*) FROM notifications.messages{where_clause}");
            let mut q = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(ref r) = recipient {
                q = q.bind(r);
            }
            match q.fetch_one(self.db.pool()).await {
                Ok(row) => row.0,
                Err(e) => {
                    error!(error = %e, "Failed to fetch total_count stats");
                    return error_result(&format!("Database error fetching stats: {e}"));
                }
            }
        };

        let total_undelivered = total_count - total_delivered;
        let delivery_rate_pct = if total_count > 0 {
            (total_delivered as f64 / total_count as f64) * 100.0
        } else {
            0.0
        };

        json_result(&NotificationStats {
            total_unread,
            unread_email,
            unread_slack,
            unread_in_app,
            total_delivered,
            total_undelivered,
            delivery_rate_pct,
        })
    }
}

// ============================================================================
// ServerHandler trait implementation
// ============================================================================

impl ServerHandler for NotificationsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "DataXLR8 Notifications MCP — send notifications, manage rules, track delivery stats"
                    .into(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        async {
            Ok(ListToolsResult {
                tools: build_tools(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            let args =
                serde_json::to_value(&request.arguments).unwrap_or(serde_json::Value::Null);
            let name_str: &str = request.name.as_ref();

            let result = match name_str {
                "send_notification" => self.handle_send_notification(&args).await,
                "list_notifications" => self.handle_list_notifications(&args).await,
                "mark_read" => match require_str_trimmed(&args, "id") {
                    Ok(id) => self.handle_mark_read(&id).await,
                    Err(e) => e,
                },
                "mark_all_read" => match require_str_trimmed(&args, "recipient") {
                    Ok(r) => self.handle_mark_all_read(&r).await,
                    Err(e) => e,
                },
                "create_rule" => self.handle_create_rule(&args).await,
                "list_rules" => self.handle_list_rules(&args).await,
                "delete_rule" => match require_str_trimmed(&args, "id") {
                    Ok(id) => self.handle_delete_rule(&id).await,
                    Err(e) => e,
                },
                "notification_stats" => self.handle_notification_stats(&args).await,
                _ => error_result(&format!("Unknown tool: {}", request.name)),
            };

            Ok(result)
        }
    }
}
