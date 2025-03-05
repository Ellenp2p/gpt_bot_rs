use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Row, Error as SqlxError};
use std::error::Error;
use crate::db::DatabasePool;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: i32,
    pub chat_id: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl Session {
    // 查找或创建会话
    pub async fn find_or_create_by_chat_id(
        pool: &DatabasePool, 
        chat_id: i64
    ) -> Result<i32, Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                // 尝试查找现有会话
                let session = sqlx::query(
                    "SELECT id FROM sessions WHERE chat_id = ?"
                )
                .bind(chat_id)
                .fetch_optional(db)
                .await?;
                
                if let Some(row) = session {
                    let id: i32 = row.get(0);
                    
                    // 更新最后活动时间
                    sqlx::query(
                        "UPDATE sessions SET updated_at = datetime('now','localtime') WHERE id = ?"
                    )
                    .bind(id)
                    .execute(db)
                    .await?;
                    
                    Ok(id)
                } else {
                    // 创建新会话
                    let result = sqlx::query(
                        "INSERT INTO sessions (chat_id) VALUES (?)"
                    )
                    .bind(chat_id)
                    .execute(db)
                    .await?;
                    
                    Ok(result.last_insert_rowid() as i32)
                }
            },
            DatabasePool::Postgres(db) => {
                // 尝试查找现有会话
                let session = sqlx::query(
                    "SELECT id FROM sessions WHERE chat_id = $1"
                )
                .bind(chat_id)
                .fetch_optional(db)
                .await?;
                
                if let Some(row) = session {
                    let id: i32 = row.get(0);
                    
                    // 更新最后活动时间
                    sqlx::query(
                        "UPDATE sessions SET updated_at = CURRENT_TIMESTAMP WHERE id = $1"
                    )
                    .bind(id)
                    .execute(db)
                    .await?;
                    
                    Ok(id)
                } else {
                    // 创建新会话
                    let row = sqlx::query(
                        "INSERT INTO sessions (chat_id) VALUES ($1) RETURNING id"
                    )
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
        chat_id: i64
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match pool {
            DatabasePool::Sqlite(db) => {
                // 获取所有相关会话
                let sessions = sqlx::query("SELECT id FROM sessions WHERE chat_id = ?")
                    .bind(chat_id)
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
            },
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
                sqlx::query(
                    "INSERT INTO messages (session_id, role, content) VALUES (?, ?, ?)"
                )
                .bind(session_id)
                .bind(role)
                .bind(content)
                .execute(db)
                .await?;
                
                Ok(())
            },
            DatabasePool::Postgres(db) => {
                sqlx::query(
                    "INSERT INTO messages (session_id, role, content) VALUES ($1, $2, $3)"
                )
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
        limit: i64
    ) -> Result<Vec<ChatMessage>, Box<dyn Error + Send + Sync>> {
        let messages = match pool {
            DatabasePool::Sqlite(db) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT role, content FROM messages 
                     WHERE session_id = ? 
                     ORDER BY timestamp ASC 
                     LIMIT ?"
                )
                .bind(session_id)
                .bind(limit)
                .fetch_all(db)
                .await?
            },
            DatabasePool::Postgres(db) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT role, content FROM messages 
                     WHERE session_id = $1 
                     ORDER BY timestamp ASC 
                     LIMIT $2"
                )
                .bind(session_id)
                .bind(limit)
                .fetch_all(db)
                .await?

            }
        };
        
        let mut chat_messages = Vec::new();
        for (role, content) in messages {
            chat_messages.push(ChatMessage {
                role,
                content,
            });
        }
        
        Ok(chat_messages)
    }
}
