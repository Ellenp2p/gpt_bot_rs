use dotenv::dotenv;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;
use std::env;
use std::error::Error;
use teloxide::{net::Download, prelude::*, types::File as TgFile, utils::command::BotCommands};

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
    #[command(description = "添加用户到白名单 (仅管理员可用)")]
    AddUser(String),
    #[command(description = "从白名单移除用户 (仅管理员可用)")]
    RemoveUser(String),
    #[command(description = "列出所有白名单用户 (仅管理员可用)")]
    ListUsers,
    #[command(description = "添加管理员 (仅超级管理员可用)")]
    AddAdmin(String),
    #[command(description = "列出所有管理员 (仅管理员可用)")]
    ListAdmins,
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

    // 设置机器人命令
    setup_commands(&bot).await?;
    log::info!("Bot commands have been set");

    let db_pool_clone = db_pool.clone();
    let openai_token_clone = openai_token.clone();

    // 更新处理器，根据消息类型分流
    let message_handler = Update::filter_message()
        .branch(
            dptree::filter(|msg: Message| msg.voice().is_some()).endpoint(
                move |bot: Bot, msg: Message| {
                    let openai_token = openai_token_clone.clone();
                    let db = db_pool_clone.clone();
                    async move {
                        // 检查白名单
                        if !check_whitelist(&bot, &msg, &db).await {
                            return respond(());
                        }

                        if let Err(err) =
                            handle_voice_message(bot.clone(), msg.clone(), &openai_token, &db).await
                        {
                            log::error!("语音处理错误: {:?}", err);
                            let _ = bot.send_message(msg.chat.id, "处理语音时发生错误").await;
                        }
                        respond(())
                    }
                },
            ),
        )
        .branch(dptree::entry().filter_command::<Command>().endpoint({
            let db = db_pool.clone();
            let openai_token = openai_token.clone();
            move |bot: Bot, msg: Message, cmd: Command| {
                let db = db.clone();
                let openai_token = openai_token.clone();
                async move { handle_command(bot, msg, cmd, &db, &openai_token).await }
            }
        }))
        .branch(
            dptree::filter(|msg: Message| msg.text().is_some()).endpoint({
                let db = db_pool.clone();
                let openai_token = openai_token.clone();
                move |bot: Bot, msg: Message| {
                    let db = db.clone();
                    let openai_token = openai_token.clone();
                    async move {
                        // 检查白名单
                        if !check_whitelist(&bot, &msg, &db).await {
                            return respond(());
                        }

                        handle_text_message(bot, msg, &db, &openai_token).await
                    }
                }
            }),
        );

    Dispatcher::builder(bot, message_handler)
        .default_handler(|upd| async move {
            log::warn!("未处理的更新: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text("处理消息时发生错误"))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

// 设置机器人命令列表
async fn setup_commands(bot: &Bot) -> Result<(), Box<dyn Error + Send + Sync>> {
    let commands = Command::bot_commands();
    bot.set_my_commands(commands).await?;
    Ok(())
}

// 检查用户是否在白名单中
async fn check_whitelist(bot: &Bot, msg: &Message, db_pool: &db::DatabasePool) -> bool {
    if let Some(user) = &msg.from {
        // 检查是否是管理员或在白名单中
        match models::Admin::is_admin(db_pool, user.id.0).await {
            Ok(true) => return true, // 管理员始终允许访问
            _ => {}
        }

        match models::WhitelistUser::is_user_whitelisted(db_pool, user.id.0).await {
            Ok(true) => return true, // 白名单用户允许访问
            Ok(false) => {
                // 用户不在白名单中，发送提示消息
                let _ = bot
                    .send_message(
                        msg.chat.id,
                        "⚠️ 您没有权限使用此机器人。请联系管理员将您添加到白名单。",
                    )
                    .await;
                return false;
            }
            Err(e) => {
                log::error!("检查白名单错误: {:?}", e);
                let _ = bot
                    .send_message(
                        msg.chat.id,
                        "检查白名单时发生错误，请稍后再试或联系管理员。",
                    )
                    .await;
                return false;
            }
        }
    } else {
        // 消息没有发送者信息
        log::warn!("消息没有发送者信息");
        let _ = bot
            .send_message(msg.chat.id, "无法识别用户信息，请联系管理员。")
            .await;
        return false;
    }
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    db_pool: &db::DatabasePool,
    openai_token: &str,
) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
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
            // 检查用户是否在白名单中
            if !check_whitelist(&bot, &msg, db_pool).await {
                return Ok(());
            }

            match models::Session::clear_history_by_chat_id(db_pool, msg.chat.id.0).await {
                Ok(_) => {
                    bot.send_message(msg.chat.id, "已清除聊天历史记录！")
                        .await?;
                }
                Err(e) => {
                    log::error!("清除历史记录错误: {:?}", e);
                    bot.send_message(msg.chat.id, "清除聊天历史时发生错误")
                        .await?;
                }
            }
        }
        Command::AddUser(arg) => {
            // 检查发送者是否是管理员
            if let Some(from) = &msg.from {
                match models::Admin::is_admin(db_pool, from.id.0).await {
                    Ok(true) => {
                        // 解析用户ID
                        match arg.trim().parse::<u64>() {
                            Ok(user_id) => {
                                // 获取可选备注
                                let parts: Vec<&str> = arg.splitn(2, ' ').collect();
                                let notes = if parts.len() > 1 {
                                    Some(parts[1])
                                } else {
                                    None
                                };

                                // 添加用户到白名单
                                match models::WhitelistUser::add_user(
                                    db_pool, user_id, None, from.id.0, notes,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        bot.send_message(
                                            msg.chat.id,
                                            format!("✅ 成功添加用户 {} 到白名单", user_id),
                                        )
                                        .await?;
                                    }
                                    Err(e) => {
                                        log::error!("添加白名单用户错误: {:?}", e);
                                        bot.send_message(msg.chat.id, "添加用户到白名单时发生错误")
                                            .await?;
                                    }
                                }
                            }
                            Err(_) => {
                                bot.send_message(
                                    msg.chat.id,
                                    "请提供有效的用户ID，格式：/adduser [用户ID] [备注]",
                                )
                                .await?;
                            }
                        }
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, "⚠️ 您没有管理员权限，无法添加白名单用户")
                            .await?;
                    }
                    Err(e) => {
                        log::error!("检查管理员权限错误: {:?}", e);
                        bot.send_message(msg.chat.id, "检查管理员权限时发生错误")
                            .await?;
                    }
                }
            }
        }
        Command::RemoveUser(arg) => {
            // 检查发送者是否是管理员
            if let Some(from) = &msg.from {
                match models::Admin::is_admin(db_pool, from.id.0).await {
                    Ok(true) => {
                        // 解析用户ID
                        match arg.trim().parse::<u64>() {
                            Ok(user_id) => {
                                // 从白名单移除用户
                                match models::WhitelistUser::remove_user(db_pool, user_id).await {
                                    Ok(true) => {
                                        bot.send_message(
                                            msg.chat.id,
                                            format!("✅ 已从白名单中移除用户 {}", user_id),
                                        )
                                        .await?;
                                    }
                                    Ok(false) => {
                                        bot.send_message(
                                            msg.chat.id,
                                            format!("⚠️ 用户 {} 不在白名单中", user_id),
                                        )
                                        .await?;
                                    }
                                    Err(e) => {
                                        log::error!("移除白名单用户错误: {:?}", e);
                                        bot.send_message(msg.chat.id, "移除用户时发生错误").await?;
                                    }
                                }
                            }
                            Err(_) => {
                                bot.send_message(
                                    msg.chat.id,
                                    "请提供有效的用户ID，格式：/removeuser [用户ID]",
                                )
                                .await?;
                            }
                        }
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, "⚠️ 您没有管理员权限，无法移除白名单用户")
                            .await?;
                    }
                    Err(e) => {
                        log::error!("检查管理员权限错误: {:?}", e);
                        bot.send_message(msg.chat.id, "检查管理员权限时发生错误")
                            .await?;
                    }
                }
            }
        }
        Command::ListUsers => {
            // 检查发送者是否是管理员
            if let Some(from) = &msg.from {
                match models::Admin::is_admin(db_pool, from.id.0).await {
                    Ok(true) => {
                        // 获取白名单用户列表
                        match models::WhitelistUser::get_all_users(db_pool).await {
                            Ok(users) => {
                                let user_list = users
                                    .iter()
                                    .map(|user| {
                                        format!("ID: {}, 备注: {:?}", user.user_id, user.notes)
                                    })
                                    .collect::<Vec<String>>()
                                    .join("\n");

                                bot.send_message(
                                    msg.chat.id,
                                    format!("白名单用户列表:\n{}", user_list),
                                )
                                .await?;
                            }
                            Err(e) => {
                                log::error!("获取白名单用户列表错误: {:?}", e);
                                bot.send_message(msg.chat.id, "获取白名单用户列表时发生错误")
                                    .await?;
                            }
                        }
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, "⚠️ 您没有管理员权限，无法查看白名单用户")
                            .await?;
                    }
                    Err(e) => {
                        log::error!("检查管理员权限错误: {:?}", e);
                        bot.send_message(msg.chat.id, "检查管理员权限时发生错误")
                            .await?;
                    }
                }
            }
        }
        Command::AddAdmin(arg) => {
            // 检查发送者是否是超级管理员
            if let Some(from) = &msg.from {
                match models::Admin::is_super_admin(db_pool, from.id.0).await {
                    Ok(true) => {
                        // 解析用户ID
                        match arg.trim().parse::<u64>() {
                            Ok(user_id) => {
                                // 添加管理员
                                match models::Admin::add_admin(db_pool, user_id, None, false).await
                                {
                                    Ok(_) => {
                                        bot.send_message(
                                            msg.chat.id,
                                            format!("✅ 成功添加管理员 {}", user_id),
                                        )
                                        .await?;
                                    }
                                    Err(e) => {
                                        log::error!("添加管理员错误: {:?}", e);
                                        bot.send_message(msg.chat.id, "添加管理员时发生错误")
                                            .await?;
                                    }
                                }
                            }
                            Err(_) => {
                                bot.send_message(
                                    msg.chat.id,
                                    "请提供有效的用户ID，格式：/addadmin [用户ID]",
                                )
                                .await?;
                            }
                        }
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, "⚠️ 您没有超级管理员权限，无法添加管理员")
                            .await?;
                    }
                    Err(e) => {
                        log::error!("检查超级管理员权限错误: {:?}", e);
                        bot.send_message(msg.chat.id, "检查超级管理员权限时发生错误")
                            .await?;
                    }
                }
            }
        }
        Command::ListAdmins => {
            // 检查发送者是否是管理员
            if let Some(from) = &msg.from {
                match models::Admin::is_admin(db_pool, from.id.0).await {
                    Ok(true) => {
                        // 获取管理员列表
                        match models::Admin::get_all_admins(db_pool).await {
                            Ok(admins) => {
                                let admin_list = admins
                                    .iter()
                                    .map(|admin| format!("ID: {}", admin.user_id))
                                    .collect::<Vec<String>>()
                                    .join("\n");

                                bot.send_message(
                                    msg.chat.id,
                                    format!("管理员列表:\n{}", admin_list),
                                )
                                .await?;
                            }
                            Err(e) => {
                                log::error!("获取管理员列表错误: {:?}", e);
                                bot.send_message(msg.chat.id, "获取管理员列表时发生错误")
                                    .await?;
                            }
                        }
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, "⚠️ 您没有管理员权限，无法查看管理员列表")
                            .await?;
                    }
                    Err(e) => {
                        log::error!("检查管理员权限错误: {:?}", e);
                        bot.send_message(msg.chat.id, "检查管理员权限时发生错误")
                            .await?;
                    }
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
    openai_token: &str,
) -> ResponseResult<()> {
    // 处理普通文本消息
    if let Some(text) = msg.text() {
        if !text.starts_with('/') {
            // 不是命令的普通文本
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
                }
                Err(e) => {
                    log::error!("GPT处理错误: {:?}", e);
                    bot.edit_message_text(
                        chat_id,
                        thinking_message.id,
                        "处理消息时发生错误，请稍后再试。",
                    )
                    .await?;
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
    api_key: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // 查找或创建会话
    let session_id = models::Session::find_or_create_by_chat_id(db_pool, chat_id).await?;

    // 保存用户消息
    models::Message::create(db_pool, session_id, "user", message).await?;

    // 获取历史消息
    let history = models::Message::get_recent_messages(db_pool, session_id, 10).await?;

    // 构建 GPT 请求
    let messages: Vec<serde_json::Value> = history
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content
            })
        })
        .collect();

    // 添加当前消息
    let all_messages = messages;

    // 调用 GPT API
    let client = reqwest::Client::builder().build()?;
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
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

async fn handle_voice_message(
    bot: Bot,
    msg: Message,
    openai_token: &str,
    db_pool: &db::DatabasePool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(voice) = msg.voice() {
        let chat_id = msg.chat.id;

        // 发送"处理中"信息
        let processing_msg = bot
            .send_message(chat_id, "正在处理您的语音消息，请稍候...")
            .await?;

        // 获取语音文件
        let file_id = &voice.file.id;
        let file = bot.get_file(file_id).await?;

        // 下载语音文件到内存
        let voice_data = download_voice(&bot, &file).await?;

        // 发送到OpenAI进行转录
        match transcribe_audio(&voice_data, openai_token).await {
            Ok(text) => {
                // 显示转录结果
                bot.edit_message_text(chat_id, processing_msg.id, format!("语音内容: {}", text))
                    .await?;

                // 将转录内容保存到数据库
                let session_id =
                    models::Session::find_or_create_by_chat_id(db_pool, chat_id.0).await?;
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
                    }
                    Err(e) => {
                        log::error!("GPT处理错误: {:?}", e);
                        bot.edit_message_text(
                            chat_id,
                            thinking_message.id,
                            "处理消息时发生错误，请稍后再试。",
                        )
                        .await?;
                    }
                }
            }
            Err(e) => {
                bot.edit_message_text(chat_id, processing_msg.id, format!("处理语音时出错: {}", e))
                    .await?;
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
async fn transcribe_audio(
    audio_data: &[u8],
    api_key: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    // 创建multipart表单
    let part = Part::bytes(audio_data.to_vec())
        .file_name("audio.oga")
        .mime_str("audio/ogg")?;

    let form = Form::new().part("file", part).text("model", "whisper-1");

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
