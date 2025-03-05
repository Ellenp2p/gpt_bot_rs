# DeepSeek AI Telegram Bot

一个功能强大的Telegram AI助手机器人，支持文本对话和语音识别功能，基于OpenAI API构建。

## 主要特性

- 💬 **智能对话**: 基于GPT-4o-mini的自然语言交流
- 🎤 **语音识别**: 支持语音消息转录并回复
- 📝 **会话记忆**: 保存对话历史，实现上下文连贯的交流
- 🔄 **多数据库支持**: 兼容SQLite和PostgreSQL
- 🧹 **清除历史**: 随时清除历史对话记录
- 🔒 **白名单管理**: 控制用户访问权限，仅允许授权用户使用机器人
- 👮 **管理员系统**: 支持多级管理权限，超级管理员可添加普通管理员

## 技术栈

- Rust + Tokio (异步运行时)
- Teloxide (Telegram Bot API框架)
- SQLx (数据库ORM)
- OpenAI API (GPT-4o-mini和Whisper)

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
git clone https://github.com/yourusername/deepseek_ai_test.git
cd deepseek_ai_test
```

2. 创建并配置环境变量文件：

```bash
cp .env.example .env
# 编辑.env文件，添加必要的API密钥和配置
```

3. 编译和运行：

```bash
cargo build --release
./target/release/deepseek_ai_test
```

## 环境变量配置

在`.env`文件中设置以下环境变量：

```
# 必需的配置
TELEGRAM_BOT_TOKEN=your_telegram_bot_token_here
OPENAI_API_KEY=your_openai_api_key_here

# 数据库配置 (默认为SQLite)
DATABASE_URL=sqlite:chat_database.db
# 或者使用PostgreSQL
# DATABASE_URL=postgres://username:password@localhost/dbname

# 管理员配置
# 可以配置多个管理员ID，用逗号分隔
ADMIN_USER_IDS=12345678,87654321,98765432
```

## 支持的命令

机器人支持以下Telegram命令：

- `/start` - 开始使用机器人
- `/help` - 显示帮助信息
- `/ping` - 测试机器人是否在线
- `/clear` - 清除聊天历史记录
- `/adduser` - 添加用户到白名单（仅管理员可用）
- `/removeuser` - 从白名单移除用户（仅管理员可用）
- `/listusers` - 列出所有白名单用户（仅管理员可用）
- `/addadmin` - 添加管理员（仅超级管理员可用）
- `/listadmins` - 列出所有管理员（仅管理员可用）

## 使用方法

1. 在Telegram中搜索您的机器人用户名
2. 发送 `/start` 命令开始对话
3. 您可以：
   - 直接发送文本消息进行对话
   - 发送语音消息，机器人会自动转录并回复
   - 使用 `/clear` 命令清除历史对话

## 白名单和管理员系统

机器人实现了两级权限系统：

1. **超级管理员**：
   - 在`.env`文件的`ADMIN_USER_IDS`变量中配置
   - 可以添加/删除普通管理员
   - 可以添加/删除白名单用户
   - 不受白名单限制

2. **普通管理员**：
   - 由超级管理员通过`/addadmin`命令添加
   - 可以添加/删除白名单用户
   - 不受白名单限制

3. **白名单用户**：
   - 由管理员通过`/adduser`命令添加
   - 可以使用机器人的所有对话功能

未在白名单中的用户将无法使用机器人功能。

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