# DeepSeek/Open AI Telegram Bot

一个功能强大的Telegram AI助手机器人，支持文本对话和语音识别功能，基于OpenAI API构建。

## 主要特性

- 💬 **智能对话**: 基于GPT-3.5 Turbo的自然语言交流
- 🎤 **语音识别**: 支持语音消息转录并回复
- 📝 **会话记忆**: 保存对话历史，实现上下文连贯的交流
- 🔄 **多数据库支持**: 兼容SQLite和PostgreSQL
- 🧹 **清除历史**: 随时清除历史对话记录

## 技术栈

- Rust + Tokio (异步运行时)
- Teloxide (Telegram Bot API框架)
- SQLx (数据库ORM)
- OpenAI API (GPT-3.5和Whisper)

## 安装指南

### 前置条件

- Rust 2021版本或更新
- Cargo包管理器
- SQLite或PostgreSQL数据库
- Telegram Bot Token (通过BotFather获取)
- OpenAI API密钥

### 安装步骤

1. 克隆代码库：

```bash
git clone https://github.com/Ellenp2p/gpt_bot_rs.git
cd gpt_bot_rs
```

2. 创建并配置环境变量文件：

```bash
cp .env.example .env
# 编辑.env文件，添加必要的API密钥和配置
```

3. 编译和运行：

```bash
cargo build --release
./target/release/gpt_bot_rs
```

## 环境变量配置

在`.env`文件中设置以下环境变量：

```
# 必需的配置
TELEGRAM_BOT_TOKEN=your_telegram_bot_token_here
OPENAI_API_KEY=your_openai_api_key_here

# 数据库配置 (默认为SQLite)
DATABASE_URL=sqlite:chat_database.db?mode=rwc
# 或者使用PostgreSQL
# DATABASE_URL=postgres://username:password@localhost/dbname
```

## 支持的命令

机器人支持以下Telegram命令：

- `/start` - 开始使用机器人
- `/help` - 显示帮助信息
- `/ping` - 测试机器人是否在线
- `/clear` - 清除聊天历史记录

## 使用方法

1. 在Telegram中搜索您的机器人用户名
2. 发送 `/start` 命令开始对话
3. 您可以：
   - 直接发送文本消息进行对话
   - 发送语音消息，机器人会自动转录并回复
   - 使用 `/clear` 命令清除历史对话

## 数据库结构

机器人使用两个主要表格：

1. `sessions` - 存储用户会话信息
2. `messages` - 存储对话消息历史

## 自定义配置

您可以通过修改以下文件来自定义机器人行为：

- `main.rs` - 主程序逻辑和消息处理
- `models.rs` - 数据模型和数据库操作
- `db.rs` - 数据库连接和初始化

## 许可证

MIT License

## 贡献指南

欢迎提交Pull Request或Issue来改进这个项目！

1. Fork本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建一个Pull Request