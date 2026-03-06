# dataxlr8-notifications-mcp

MCP server for sending notifications across multiple channels (email, Slack, in-app) with priority levels and auto-notification rules. Supports rule creation for events like deal movement, task overdue, and score thresholds.

## Tools

| Tool | Description |
|------|-------------|
| send_notification | Create a notification for a user/channel (email, slack, in_app) with priority |
| list_notifications | List notifications with optional filters by recipient, channel, priority, read status |
| mark_read | Mark a notification as read |
| mark_all_read | Mark all notifications for a user as read |
| create_rule | Create an auto-notification rule triggered by events (deal_moved, task_overdue, score_threshold) |
| list_rules | List notification rules, optionally filtered by event type or active status |
| delete_rule | Delete a notification rule |
| notification_stats | Get notification stats: unread counts by channel, delivery rates |

## Setup

```bash
DATABASE_URL=postgres://dataxlr8:dataxlr8@localhost:5432/dataxlr8 cargo run
```

## Schema

Creates `notifications.*` schema in PostgreSQL with tables:
- `notifications.messages` - Notification messages with delivery tracking
- `notifications.rules` - Auto-notification rules

## Part of

[DataXLR8](https://github.com/pdaxt) - AI-powered recruitment platform
