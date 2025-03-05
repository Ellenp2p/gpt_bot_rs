use crate::db::DatabasePool;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Error as SqlxError, Row};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: i32,
    pub chat_id: u64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WhitelistUser {
    pub id: i32,
    pub user_id: u64,
    pub username: Option<String>,
    pub added_by: u64,
    pub added_at: NaiveDateTime,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Admin {
    pub id: i32,
    pub user_id: u64,
    pub username: Option<String>,
    pub is_super: bool,
    pub added_at: NaiveDateTime,
}

impl Session {
    // 查找或创建会话
    pub async fn find_or_create_by_chat_id(
        pool: &DatabasePool,
        chat_id: i64,
    ) -> Result<i32, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                // 尝试查找现有会话
                let session = sqlx::query("SELECT id FROM sessions WHERE chat_id = ?")
                    .bind(chat_id)
                    .fetch_optional(db)
                    .await?;

                if let Some(row) = session {
                    let id: i32 = row.get(0);

                    // 更新最后活动时间
                    sqlx::query(
                        "UPDATE sessions SET updated_at = datetime('now','localtime') WHERE id = ?",
                    )
                    .bind(id)
                    .execute(db)
                    .await?;

                    Ok(id)
                } else {
                    // 创建新会话
                    let result = sqlx::query("INSERT INTO sessions (chat_id) VALUES (?)")
                        .bind(chat_id)
                        .execute(db)
                        .await?;

                    Ok(result.last_insert_rowid() as i32)
                }
            }
            DatabasePool::Postgres(db) => {
                // 尝试查找现有会话
                let session = sqlx::query("SELECT id FROM sessions WHERE chat_id = $1")
                    .bind(chat_id)
                    .fetch_optional(db)
                    .await?;

                if let Some(row) = session {
                    let id: i32 = row.get(0);

                    // 更新最后活动时间
                    sqlx::query("UPDATE sessions SET updated_at = CURRENT_TIMESTAMP WHERE id = $1")
                        .bind(id)
                        .execute(db)
                        .await?;

                    Ok(id)
                } else {
                    // 创建新会话
                    let row =
                        sqlx::query("INSERT INTO sessions (chat_id) VALUES ($1) RETURNING id")
                            .bind(chat_id)
                            .fetch_one(db)
                            .await?;

                    Ok(row.get(0))
                }
            }
        }
    }

    // 清除聊天历史
    pub async fn clear_history_by_chat_id(
        pool: &DatabasePool,
        chat_id: i64,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                // 获取所有相关会话
                let sessions = sqlx::query("SELECT id FROM sessions WHERE chat_id = ?")
                    .bind(chat_id as i64)
                    .fetch_all(db)
                    .await?;

                // 删除所有相关消息
                for row in &sessions {
                    let id: i32 = row.get(0);
                    sqlx::query("DELETE FROM messages WHERE session_id = ?")
                        .bind(id)
                        .execute(db)
                        .await?;
                }

                // 删除所有相关会话
                sqlx::query("DELETE FROM sessions WHERE chat_id = ?")
                    .bind(chat_id)
                    .execute(db)
                    .await?;

                Ok(())
            }
            DatabasePool::Postgres(db) => {
                // 获取所有相关会话
                let sessions = sqlx::query("SELECT id FROM sessions WHERE chat_id = $1")
                    .bind(chat_id)
                    .fetch_all(db)
                    .await?;

                // 删除所有相关消息
                for row in &sessions {
                    let id: i32 = row.get(0);
                    sqlx::query("DELETE FROM messages WHERE session_id = $1")
                        .bind(id)
                        .execute(db)
                        .await?;
                }

                // 删除所有相关会话
                sqlx::query("DELETE FROM sessions WHERE chat_id = $1")
                    .bind(chat_id)
                    .execute(db)
                    .await?;

                Ok(())
            }
        }
    }
}

pub struct Message;

impl Message {
    // 创建新消息
    pub async fn create(
        pool: &DatabasePool,
        session_id: i32,
        role: &str,
        content: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                sqlx::query("INSERT INTO messages (session_id, role, content) VALUES (?, ?, ?)")
                    .bind(session_id)
                    .bind(role)
                    .bind(content)
                    .execute(db)
                    .await?;

                Ok(())
            }
            DatabasePool::Postgres(db) => {
                sqlx::query("INSERT INTO messages (session_id, role, content) VALUES ($1, $2, $3)")
                    .bind(session_id)
                    .bind(role)
                    .bind(content)
                    .execute(db)
                    .await?;

                Ok(())
            }
        }
    }

    // 获取最近消息
    pub async fn get_recent_messages(
        pool: &DatabasePool,
        session_id: i32,
        limit: i64,
    ) -> Result<Vec<ChatMessage>, Box<dyn Error + Send + Sync>> {
        let messages = match pool {
            DatabasePool::Sqlite(db) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT role, content FROM messages 
                     WHERE session_id = ? 
                     ORDER BY timestamp ASC 
                     LIMIT ?",
                )
                .bind(session_id)
                .bind(limit)
                .fetch_all(db)
                .await?
            }
            DatabasePool::Postgres(db) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT role, content FROM messages 
                     WHERE session_id = $1 
                     ORDER BY timestamp ASC 
                     LIMIT $2",
                )
                .bind(session_id)
                .bind(limit)
                .fetch_all(db)
                .await?
            }
        };

        let mut chat_messages = Vec::new();
        for (role, content) in messages {
            chat_messages.push(ChatMessage { role, content });
        }

        Ok(chat_messages)
    }
}

impl WhitelistUser {
    // 检查用户是否在白名单中
    pub async fn is_user_whitelisted(
        pool: &DatabasePool,
        user_id: u64,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let result =
                    sqlx::query("SELECT COUNT(*) as count FROM whitelist_users WHERE user_id = ?")
                        .bind(user_id as i64)
                        .fetch_one(db)
                        .await?;

                let count: u64 = result.get(0);
                Ok(count > 0)
            }
            DatabasePool::Postgres(db) => {
                let result =
                    sqlx::query("SELECT COUNT(*) as count FROM whitelist_users WHERE user_id = $1")
                        .bind(user_id as i64)
                        .fetch_one(db)
                        .await?;

                let count: i64 = result.get(0);
                Ok(count > 0)
            }
        }
    }

    // 添加用户到白名单
    pub async fn add_user(
        pool: &DatabasePool,
        user_id: u64,
        username: Option<&str>,
        added_by: u64,
        notes: Option<&str>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO whitelist_users (user_id, username, added_by, notes) VALUES (?, ?, ?, ?)"
                )
                .bind(user_id as i64)
                .bind(username)
                .bind(added_by as i64)
                .bind(notes)
                .execute(db)
                .await?;

                Ok(())
            }
            DatabasePool::Postgres(db) => {
                sqlx::query(
                    "INSERT INTO whitelist_users (user_id, username, added_by, notes) VALUES ($1, $2, $3, $4) ON CONFLICT (user_id) DO NOTHING"
                )
                .bind(user_id as i64)
                .bind(username)
                .bind(added_by as i64)
                .bind(notes)
                .execute(db)
                .await?;

                Ok(())
            }
        }
    }

    // 从白名单移除用户
    pub async fn remove_user(
        pool: &DatabasePool,
        user_id: u64,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let result = sqlx::query("DELETE FROM whitelist_users WHERE user_id = ?")
                    .bind(user_id as i64)
                    .execute(db)
                    .await?;

                Ok(result.rows_affected() > 0)
            }
            DatabasePool::Postgres(db) => {
                let result = sqlx::query("DELETE FROM whitelist_users WHERE user_id = $1")
                    .bind(user_id as i64)
                    .execute(db)
                    .await?;

                Ok(result.rows_affected() > 0)
            }
        }
    }

    // 获取所有白名单用户
    pub async fn get_all_users(
        pool: &DatabasePool,
    ) -> Result<Vec<WhitelistUser>, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let rows: Vec<WhitelistUser> = sqlx::query(
                    "SELECT id, user_id, username, added_by, added_at, notes FROM whitelist_users ORDER BY added_at DESC"
                )
                .map(|row: sqlx::sqlite::SqliteRow| {
                    WhitelistUser {
                        id: row.get(0),
                        user_id: row.get::<i64, _>(1) as u64,
                        username: row.get(2),
                        added_by: row.get::<i64, _>(3) as u64,
                        added_at: row.get(4),
                        notes: row.get(5),
                    }
                })
                .fetch_all(db)
                .await?;

                Ok(rows)
            }
            DatabasePool::Postgres(db) => {
                let rows: Vec<WhitelistUser> = sqlx::query(
                    "SELECT id, user_id, username, added_by, added_at, notes FROM whitelist_users ORDER BY added_at DESC"
                )
                .map(|row: sqlx::postgres::PgRow| {
                    WhitelistUser {
                        id: row.get(0),
                        user_id: row.get::<i64, _>(1) as u64,
                        username: row.get(2),
                        added_by: row.get::<i64, _>(3) as u64,
                        added_at: row.get(4),
                        notes: row.get(5),
                    }
                })
                .fetch_all(db)
                .await?;

                Ok(rows)
            }
        }
    }
}

impl Admin {
    // 检查用户是否是管理员
    pub async fn is_admin(
        pool: &DatabasePool,
        user_id: u64,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let result = sqlx::query("SELECT COUNT(*) as count FROM admins WHERE user_id = ?")
                    .bind(user_id as i64)
                    .fetch_one(db)
                    .await?;

                let count: u64 = result.get(0);
                Ok(count > 0)
            }
            DatabasePool::Postgres(db) => {
                let result = sqlx::query("SELECT COUNT(*) as count FROM admins WHERE user_id = $1")
                    .bind(user_id as i64)
                    .fetch_one(db)
                    .await?;

                let count: i64 = result.get(0);
                Ok(count > 0)
            }
        }
    }

    // 检查用户是否是超级管理员
    pub async fn is_super_admin(
        pool: &DatabasePool,
        user_id: u64,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let result = sqlx::query(
                    "SELECT COUNT(*) as count FROM admins WHERE user_id = ? AND is_super = 1",
                )
                .bind(user_id as i64)
                .fetch_one(db)
                .await?;

                let count: u64 = result.get(0);
                Ok(count > 0)
            }
            DatabasePool::Postgres(db) => {
                let result = sqlx::query(
                    "SELECT COUNT(*) as count FROM admins WHERE user_id = $1 AND is_super = TRUE",
                )
                .bind(user_id as i64)
                .fetch_one(db)
                .await?;

                let count: i64 = result.get(0);
                Ok(count > 0)
            }
        }
    }

    // 添加管理员
    pub async fn add_admin(
        pool: &DatabasePool,
        user_id: u64,
        username: Option<&str>,
        is_super: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO admins (user_id, username, is_super) VALUES (?, ?, ?)",
                )
                .bind(user_id as i64)
                .bind(username)
                .bind(is_super as i32)
                .execute(db)
                .await?;

                Ok(())
            }
            DatabasePool::Postgres(db) => {
                sqlx::query(
                    "INSERT INTO admins (user_id, username, is_super) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO NOTHING"
                )
                .bind(user_id as i64)
                .bind(username)
                .bind(is_super)
                .execute(db)
                .await?;

                Ok(())
            }
        }
    }

    // 获取所有管理员
    pub async fn get_all_admins(
        pool: &DatabasePool,
    ) -> Result<Vec<Admin>, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                let rows: Vec<Admin> = sqlx::query(
                    "SELECT id, user_id, username, is_super, added_at FROM admins ORDER BY is_super DESC, added_at ASC"
                )
                .map(|row: sqlx::sqlite::SqliteRow| {
                    Admin {
                        id: row.get(0),
                        user_id: row.get::<i64, _>(1) as u64,
                        username: row.get(2),
                        is_super: row.get::<i64, _>(3) != 0,
                        added_at: row.get(4),
                    }
                })
                .fetch_all(db)
                .await?;

                Ok(rows)
            }
            DatabasePool::Postgres(db) => {
                let rows: Vec<Admin> = sqlx::query(
                    "SELECT id, user_id, username, is_super, added_at FROM admins ORDER BY is_super DESC, added_at ASC"
                )
                .map(|row: sqlx::postgres::PgRow| {
                    Admin {
                        id: row.get(0),
                        user_id: row.get::<i64, _>(1) as u64,
                        username: row.get(2),
                        is_super: row.get(3),
                        added_at: row.get(4),
                    }
                })
                .fetch_all(db)
                .await?;

                Ok(rows)
            }
        }
    }
}
