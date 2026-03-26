//! 命令行参数定义 v0.3

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "auditlens", version, about = "AuditLens — 会计智能查账系统")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 执行审计检查（命令行模式）
    Audit {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long, value_enum)]
        format: InputFormat,
        #[arg(short, long)]
        config: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "terminal")]
        export_format: ExportFormat,
        /// 启用 AI 分析（需配置 API Key）
        #[arg(long)]
        ai: bool,
        /// AI 提供商: openai / deepseek / qwen / local
        #[arg(long, default_value = "local")]
        ai_provider: String,
        /// AI API Key
        #[arg(long, default_value = "")]
        ai_key: String,
    },
    /// 启动 Web 服务器
    Web {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// 验证配置文件
    ValidateConfig {
        #[arg(short, long)]
        config: PathBuf,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum InputFormat { Csv, Excel }

#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat { Terminal, Json, Csv, Pdf }
