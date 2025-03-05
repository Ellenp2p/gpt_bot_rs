use dotenv::dotenv;
use std::env;
use std::error::Error; 
use teloxide::{prelude::*, net::Download, types::File as TgFile, utils::command::BotCommands};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;

// 引入模块
mod db;
mod models;

// OpenAI响应结构
#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    text: String,
}

// 定义命令
#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "支持的命令：",
    parse_with = "split"
)]
enum Command {
    #[command(description = "显示帮助信息")]
    Help,
    #[command(description = "开始使用机器人")]
    Start,
    #[command(description = "测试机器人是否在线")]
    Ping,
    #[command(description = "清除聊天历史记录")]
    Clear,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // 加载环境变量
    dotenv().ok();
    
    // 获取环境变量
    let tg_token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not found");
    let openai_token = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not found");
    
    // 初始化日志
    pretty_env_logger::init();
    log::info!("Starting telegram bot...");
    
    // 初始化数据库
    let db_pool = db::init_db().await?;
    log::info!("Database initialized successfully");
    
    // 创建机器人
    let bot = Bot::new(tg_token);
    let db_pool_clone = db_pool.clone();
    let openai_token_clone = openai_token.clone();
    
    // 更新处理器，根据消息类型分流
    let message_handler = Update::filter_message()
        .branch(
            dptree::filter(|msg: Message| msg.voice().is_some())
                .endpoint(move |bot: Bot, msg: Message| {
                    let openai_token = openai_token_clone.clone();
                    let db = db_pool_clone.clone();
                    async move {
                        if let Err(err) = handle_voice_message(bot.clone(), msg.clone(), &openai_token, &db).await {
                            log::error!("语音处理错误: {:?}", err);
                            let _ = bot.send_message(msg.chat.id, "处理语音时发生错误").await;
                        }
                        respond(())
                    }
                }),
        )
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint({
                    let db = db_pool.clone();
                    let openai_token = openai_token.clone();
                    move |bot: Bot, msg: Message, cmd: Command| {
                        let db = db.clone();
                        let openai_token = openai_token.clone();
                        async move {
                            handle_command(bot, msg, cmd, &db, &openai_token).await
                        }
                    }
                }),
        )
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some())
                .endpoint({
                    let db = db_pool.clone();
                    let openai_token = openai_token.clone();
                    move |bot: Bot, msg: Message| {
                        let db = db.clone();
                        let openai_token = openai_token.clone();
                        async move {
                            handle_text_message(bot, msg, &db, &openai_token).await
                        }
                    }
                }),
        );
    
    Dispatcher::builder(bot, message_handler)
        .default_handler(|upd| async move {
            log::warn!("未处理的更新: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text(
            "处理消息时发生错误",
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    
    Ok(())
}

async fn handle_command(
    bot: Bot, 
    msg: Message, 
    cmd: Command, 
    db_pool: &db::DatabasePool, 
    openai_token: &str
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(
                msg.chat.id,
                Command::descriptions().to_string(),
            )
            .await?;
        }
        Command::Start => {
            bot.send_message(
                msg.chat.id,
                "👋 欢迎使用AI聊天机器人!\n\n你可以直接发送文字与我对话，或发送语音消息让我转录。\n使用 /help 查看所有命令。",
            )
            .await?;
        }
        Command::Ping => {
            bot.send_message(msg.chat.id, "我在线！").await?;
        }
        Command::Clear => {
            match models::Session::clear_history_by_chat_id(db_pool, msg.chat.id.0).await {
                Ok(_) => {
                    bot.send_message(msg.chat.id, "已清除聊天历史记录！").await?;
                },
                Err(e) => {
                    log::error!("清除历史记录错误: {:?}", e);
                    bot.send_message(msg.chat.id, "清除聊天历史时发生错误").await?;
                }
            }
        }
    };

    Ok(())
}

async fn handle_text_message(
    bot: Bot, 
    msg: Message, 
    db_pool: &db::DatabasePool, 
    openai_token: &str
) -> ResponseResult<()> {
    // 处理普通文本消息
    if let Some(text) = msg.text() {
        if !text.starts_with('/') { // 不是命令的普通文本
            // 显示"正在思考"的提示
            let chat_id = msg.chat.id;
            let thinking_message = bot.send_message(chat_id, "🤔 思考中...").await?;
            
            // 处理消息并获取回复
            match process_chat_message(db_pool, chat_id.0, text, openai_token).await {
                Ok(response) => {
                    // 删除"思考中"的消息
                    bot.delete_message(chat_id, thinking_message.id).await?;
                    
                    // 发送AI回复
                    bot.send_message(chat_id, response).await?;
                },
                Err(e) => {
                    log::error!("GPT处理错误: {:?}", e);
                    bot.edit_message_text(chat_id, thinking_message.id, "处理消息时发生错误，请稍后再试。").await?;
                }
            }
        }
    }
    Ok(())
}

async fn process_chat_message(
    db_pool: &db::DatabasePool, 
    chat_id: i64, 
    message: &str, 
    api_key: &str
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // 查找或创建会话
    let session_id = models::Session::find_or_create_by_chat_id(db_pool, chat_id).await?;
    
    // 保存用户消息
    models::Message::create(db_pool, session_id, "user", message).await?;
    
    // 获取历史消息
    let history = models::Message::get_recent_messages(db_pool, session_id, 10).await?;
    
    // 构建 GPT 请求
    let messages: Vec<serde_json::Value> = history.iter().map(|msg| {
        serde_json::json!({
            "role": msg.role,
            "content": msg.content
        })
    }).collect();
    
    // 添加当前消息
    let mut all_messages = messages;
    
    // 调用 GPT API
    let client = reqwest::Client::builder().build()?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": "gpt-3.5-turbo",
            "messages": all_messages,
            "temperature": 0.7
        }))
        .send()
        .await?;
    
    // 处理 GPT 响应
    if response.status().is_success() {
        let json: Value = response.json().await?;
        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
            // 保存 AI 回复
            models::Message::create(db_pool, session_id, "assistant", content).await?;
            Ok(content.to_string())
        } else {
            Err("无法解析 GPT 响应".into())
        }
    } else {
        let error_text = response.text().await?;
        Err(format!("GPT API 错误: {}", error_text).into())
    }
}

async fn handle_voice_message(bot: Bot, msg: Message, openai_token: &str, db_pool: &db::DatabasePool) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(voice) = msg.voice() {
        let chat_id = msg.chat.id;
        
        // 发送"处理中"信息
        let processing_msg = bot.send_message(chat_id, "正在处理您的语音消息，请稍候...").await?;
        
        // 获取语音文件
        let file_id = &voice.file.id;
        let file = bot.get_file(file_id).await?;
        
        // 下载语音文件到内存
        let voice_data = download_voice(&bot, &file).await?;
        
        // 发送到OpenAI进行转录
        match transcribe_audio(&voice_data, openai_token).await {
            Ok(text) => {
                // 显示转录结果
                bot.edit_message_text(chat_id, processing_msg.id, format!("语音内容: {}", text)).await?;
                
                // 将转录内容保存到数据库
                let session_id = models::Session::find_or_create_by_chat_id(db_pool, chat_id.0).await?;
                models::Message::create(db_pool, session_id, "user", &text).await?;
                
                // 显示"正在思考"的提示
                let thinking_message = bot.send_message(chat_id, "🤔 思考中...").await?;
                
                // 处理消息并获取回复
                match process_chat_message(db_pool, chat_id.0, &text, openai_token).await {
                    Ok(response) => {
                        // 删除"思考中"的消息
                        bot.delete_message(chat_id, thinking_message.id).await?;
                        
                        // 发送AI回复
                        bot.send_message(chat_id, response).await?;
                    },
                    Err(e) => {
                        log::error!("GPT处理错误: {:?}", e);
                        bot.edit_message_text(chat_id, thinking_message.id, "处理消息时发生错误，请稍后再试。").await?;
                    }
                }
            },
            Err(e) => {
                bot.edit_message_text(chat_id, processing_msg.id, format!("处理语音时出错: {}", e)).await?;
            }
        }
    }
    
    Ok(())
}

/// 将文件下载到内存而不是保存为文件
async fn download_voice(bot: &Bot, file: &TgFile) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    // 创建内存缓冲区
    let mut buffer = Vec::new();
    
    // 下载文件到内存
    bot.download_file(&file.path, &mut buffer).await?;
    
    Ok(buffer)
}

/// 从内存数据中转录音频
async fn transcribe_audio(audio_data: &[u8], api_key: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    // 创建multipart表单
    let part = Part::bytes(audio_data.to_vec())
        .file_name("audio.oga")
        .mime_str("audio/ogg")?;
    
    let form = Form::new()
        .part("file", part)
        .text("model", "whisper-1");
    
    // 发送请求到OpenAI
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    
    // 处理响应
    if response.status().is_success() {
        let json: Value = response.json().await?;
        if let Some(text) = json["text"].as_str() {
            Ok(text.to_string())
        } else {
            Err("无法获取文字内容".into())
        }
    } else {
        let error_text = response.text().await?;
        Err(format!("API错误: {}", error_text).into())
    }
}
