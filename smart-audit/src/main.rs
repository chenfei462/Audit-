//! AuditLens v0.3 入口

use anyhow::{Context, Result};
use auditlens::cli::{Cli, Commands, ExportFormat, InputFormat};
use auditlens::data_parser::csv_parser::CsvParser;
use auditlens::data_parser::excel_parser::ExcelParser;
use auditlens::data_parser::DataSource;
use auditlens::report_gen::csv_export::CsvExporter;
use auditlens::report_gen::json_export::JsonExporter;
use auditlens::report_gen::terminal::TerminalReporter;
use auditlens::report_gen::{ReportExport, ReportOutput};
use auditlens::pdf_report::PdfReportGenerator;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
       Commands::Audit { input, format, config, output, export_format, .. } => {
            println!("AuditLens v{} — 会计智能查账系统", env!("CARGO_PKG_VERSION"));
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            let config_path = config.as_deref();
            let audit_config = auditlens::load_config(config_path).context("加载配置失败")?;
            println!("正在解析数据...");
            let transactions = match format {
                InputFormat::Csv => CsvParser.parse(&input)?,
                InputFormat::Excel => ExcelParser.parse(&input)?,
            };
            println!("成功解析 {} 条交易记录", transactions.len());
            if transactions.is_empty() { println!("⚠️ 无数据"); return Ok(()); }

            let report = auditlens::run_audit(&transactions, &audit_config)?;
            match export_format {
                ExportFormat::Terminal => { TerminalReporter.print_report(&report)?; }
                ExportFormat::Json => {
                    let p = output.as_deref().unwrap_or(std::path::Path::new("audit_report.json"));
                    JsonExporter.export(&report, p)?;
                    TerminalReporter.print_report(&report)?;
                }
                ExportFormat::Csv => {
                    let p = output.as_deref().unwrap_or(std::path::Path::new("audit_report.csv"));
                    CsvExporter.export(&report, p)?;
                    TerminalReporter.print_report(&report)?;
                }
                ExportFormat::Pdf => {
                    let p = output.as_deref().unwrap_or(std::path::Path::new("audit_report.pdf"));
                    PdfReportGenerator::generate(&report, p)?;
                    TerminalReporter.print_report(&report)?;
                }
            }

            // 生成 PDF 报告
            let pdf_path = output.as_deref().unwrap_or(std::path::Path::new("audit_report.pdf"));
            let _ = PdfReportGenerator::generate(&report, pdf_path);

            // 生成本地AI分析
            println!("\n{}", auditlens::ai_analysis::generate_local_analysis(&report));

            Ok(())
        }
        Commands::Web { host, port } => {
            auditlens::web::start_server(&host, port).await?;
            Ok(())
        }
        Commands::ValidateConfig { config } => {
            println!("验证配置: {}", config.display());
            match auditlens::load_config(Some(&config)) {
                Ok(_) => { println!("✅ 配置正确"); Ok(()) }
                Err(e) => { println!("❌ {}", e); Err(e) }
            }
        }
    }
}
