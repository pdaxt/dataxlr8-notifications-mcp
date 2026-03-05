use anyhow::Result;
use sqlx::PgPool;

pub async fn setup_schema(pool: &PgPool) -> Result<()> {
    sqlx::raw_sql(
        r#"
        CREATE SCHEMA IF NOT EXISTS notifications;

        CREATE TABLE IF NOT EXISTS notifications.messages (
            id          TEXT PRIMARY KEY,
            channel     TEXT NOT NULL CHECK (channel IN ('email', 'slack', 'in_app')),
            recipient   TEXT NOT NULL,
            title       TEXT NOT NULL,
            body        TEXT NOT NULL,
            priority    TEXT NOT NULL DEFAULT 'normal'
                        CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
            read        BOOLEAN NOT NULL DEFAULT false,
            delivered   BOOLEAN NOT NULL DEFAULT false,
            created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
        );

        CREATE TABLE IF NOT EXISTS notifications.rules (
            id          TEXT PRIMARY KEY,
            event_type  TEXT NOT NULL CHECK (event_type IN ('deal_moved', 'task_overdue', 'score_threshold')),
            condition   JSONB NOT NULL DEFAULT '{}',
            channel     TEXT NOT NULL CHECK (channel IN ('email', 'slack', 'in_app')),
            template    TEXT NOT NULL,
            active      BOOLEAN NOT NULL DEFAULT true,
            created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
        );

        CREATE INDEX IF NOT EXISTS idx_messages_recipient ON notifications.messages(recipient);
        CREATE INDEX IF NOT EXISTS idx_messages_channel ON notifications.messages(channel);
        CREATE INDEX IF NOT EXISTS idx_messages_read ON notifications.messages(read);
        CREATE INDEX IF NOT EXISTS idx_messages_priority ON notifications.messages(priority);
        CREATE INDEX IF NOT EXISTS idx_messages_created ON notifications.messages(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_rules_event_type ON notifications.rules(event_type);
        CREATE INDEX IF NOT EXISTS idx_rules_active ON notifications.rules(active);
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
