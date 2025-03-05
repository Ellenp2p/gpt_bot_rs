use sqlx::{Pool, Sqlite, Postgres, Error as SqlxError};
use std::env;
use std::error::Error;

#[derive(Clone)]
pub enum DatabasePool {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
}

impl DatabasePool {
    // 执行无返回值的SQL查询
    pub async fn execute(&self, query: &str) -> Result<(), SqlxError> {
        match self {
            DatabasePool::Sqlite(pool) => {
                sqlx::query(query)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::Postgres(pool) => {
                sqlx::query(query)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

}

// 初始化数据库
pub async fn init_db() -> Result<DatabasePool, Box<dyn Error + Send + Sync>> {
    // 从环境变量获取数据库 URL
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:chat_database.db".to_string());
    
    log::info!("正在连接数据库: {}", database_url);
    
    // 判断使用哪种数据库
    if database_url.starts_with("postgres:") {
        // PostgreSQL
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        
        // 创建表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id SERIAL PRIMARY KEY,
                chat_id BIGINT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"
        ).execute(&pool).await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id SERIAL PRIMARY KEY,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )"
        ).execute(&pool).await?;
        
        log::info!("PostgreSQL 数据库初始化完成");
        Ok(DatabasePool::Postgres(pool))
    } else {
        // SQLite
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        
        // 创建表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chat_id INTEGER NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT (datetime('now','localtime'))
            )"
        ).execute(&pool).await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TIMESTAMP DEFAULT (datetime('now','localtime')),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )"
        ).execute(&pool).await?;
        
        log::info!("SQLite 数据库初始化完成");
        Ok(DatabasePool::Sqlite(pool))
    }
}
