use sqlx::{Error as SqlxError, Pool, Postgres, Sqlite};
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
                sqlx::query(query).execute(pool).await?;
            }
            DatabasePool::Postgres(pool) => {
                sqlx::query(query).execute(pool).await?;
            }
        }
        Ok(())
    }
}

// 初始化数据库
pub async fn init_db() -> Result<DatabasePool, Box<dyn Error + Send + Sync>> {
    // 从环境变量获取数据库 URL
    let database_url =
        env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:chat_database.db".to_string());

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
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id SERIAL PRIMARY KEY,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&pool)
        .await?;

        // 创建白名单表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS whitelist_users (
                id SERIAL PRIMARY KEY,
                user_id BIGINT NOT NULL UNIQUE,
                username TEXT,
                added_by BIGINT NOT NULL,
                added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                notes TEXT
            )",
        )
        .execute(&pool)
        .await?;

        // 创建管理员表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS admins (
                id SERIAL PRIMARY KEY,
                user_id BIGINT NOT NULL UNIQUE,
                username TEXT,
                is_super BOOLEAN DEFAULT FALSE,
                added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&pool)
        .await?;

        // 添加初始管理员
        let pool_ref = &DatabasePool::Postgres(pool.clone());
        add_initial_admins(pool_ref).await?;

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
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TIMESTAMP DEFAULT (datetime('now','localtime')),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            )",
        )
        .execute(&pool)
        .await?;

        // 创建白名单表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS whitelist_users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL UNIQUE,
                username TEXT,
                added_by INTEGER NOT NULL,
                added_at TIMESTAMP DEFAULT (datetime('now','localtime')),
                notes TEXT
            )",
        )
        .execute(&pool)
        .await?;

        // 创建管理员表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS admins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL UNIQUE,
                username TEXT,
                is_super INTEGER DEFAULT 0, 
                added_at TIMESTAMP DEFAULT (datetime('now','localtime'))
            )",
        )
        .execute(&pool)
        .await?;
        // 添加初始管理员
        let pool_ref = &DatabasePool::Sqlite(pool.clone());
        add_initial_admins(pool_ref).await?;

        log::info!("SQLite 数据库初始化完成");
        Ok(DatabasePool::Sqlite(pool))
    }
}

// 添加初始管理员
async fn add_initial_admins(pool: &DatabasePool) -> Result<(), Box<dyn Error + Send + Sync>> {
    // 从环境变量获取初始管理员ID
    let admin_ids = env::var("ADMIN_USER_IDS").unwrap_or_else(|_| "".to_string());

    if admin_ids.is_empty() {
        log::warn!("未设置 ADMIN_USER_IDS 环境变量，没有初始管理员");
        return Ok(());
    }

    // 处理可能有多个管理员ID的情况，以逗号分隔
    for admin_id_str in admin_ids.split(',') {
        if let Ok(admin_id) = admin_id_str.trim().parse::<i64>() {
            // 根据数据库类型选择适当的SQL查询
            match pool {
                DatabasePool::Sqlite(sqlite_pool) => {
                    sqlx::query("INSERT OR IGNORE INTO admins (user_id, is_super) VALUES (?, 1)")
                        .bind(admin_id)
                        .execute(sqlite_pool)
                        .await?;
                }
                DatabasePool::Postgres(pg_pool) => {
                    sqlx::query("INSERT INTO admins (user_id, is_super) VALUES ($1, TRUE) ON CONFLICT (user_id) DO NOTHING")
                        .bind(admin_id)
                        .execute(pg_pool)
                        .await?;
                }
            }
            log::info!("已确保初始超级管理员存在: {}", admin_id);
        } else {
            log::warn!("无效的管理员ID: {}", admin_id_str);
        }
    }
    Ok(())
}
