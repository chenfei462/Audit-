//! 终端彩色报告输出
//!
//! 使用 colored 库在终端中以彩色分级方式展示审计结果：
//! - 绿色：正常/信息
//! - 黄色：警告
//! - 红色：错误

use super::ReportOutput;
use crate::models::{AuditReport, Severity};
use anyhow::Result;
use colored::*;

/// 终端报告输出器
pub struct TerminalReporter;

impl ReportOutput for TerminalReporter {
    fn print_report(&self, report: &AuditReport) -> Result<()> {
        // 标题
        println!();
        println!("{}", "═══════════════════════════════════════════════════════".blue().bold());
        println!("{}", "              AuditLens 审计报告".blue().bold());
        println!("{}", "═══════════════════════════════════════════════════════".blue().bold());
        println!();

        // 概览信息
        println!("{}", "【审计概览】".cyan().bold());
        println!("  审计时间：{}", report.audit_time);
        println!("  交易总数：{}", report.total_transactions);
        println!("  借方总额：{}", report.total_debit);
        println!("  贷方总额：{}", report.total_credit);
        println!();

        // 统计摘要
        println!("{}", "【统计摘要】".cyan().bold());
        let errors = report.error_count();
        let warnings = report.warning_count();
        let infos = report.info_count();

        println!(
            "  {} {}  {} {}  {} {}",
            "错误:".red().bold(),
            errors.to_string().red(),
            "警告:".yellow().bold(),
            warnings.to_string().yellow(),
            "信息:".green().bold(),
            infos.to_string().green(),
        );
        println!();

        if report.findings.is_empty() {
            println!("{}", "  ✅ 未发现异常，账目审计通过！".green().bold());
            println!();
            return Ok(());
        }

        // 详细发现
        println!("{}", "【审计发现明细】".cyan().bold());
        println!("{}", "───────────────────────────────────────────────────────".dimmed());

        for (i, finding) in report.findings.iter().enumerate() {
            let severity_tag = match finding.severity {
                Severity::Error => "[错误]".red().bold(),
                Severity::Warning => "[警告]".yellow().bold(),
                Severity::Info => "[信息]".green().bold(),
            };

            println!("  {}  #{}", severity_tag, i + 1);
            println!("  规则：{}", finding.rule_name);
            println!("  说明：{}", finding.message);
            if !finding.related_rows.is_empty() {
                let rows: Vec<String> = finding.related_rows.iter().map(|r| r.to_string()).collect();
                println!("  涉及行：{}", rows.join(", "));
            }
            println!("{}", "  · · · · · · · · · · · · · · · · · · · · · · · · · ·".dimmed());
        }

        println!();
        println!("{}", "═══════════════════════════════════════════════════════".blue().bold());

        // 结论
        if errors > 0 {
            println!("{}", "  ❌ 审计发现严重问题，请立即核查！".red().bold());
        } else if warnings > 0 {
            println!("{}", "  ⚠️  审计发现警告事项，建议进一步核实。".yellow().bold());
        } else {
            println!("{}", "  ✅ 审计通过，仅有信息提示。".green().bold());
        }
        println!();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuditFinding, AuditReport, Severity};
    use rust_decimal_macros::dec;

    #[test]
    fn test_terminal_report_empty() {
        let report = AuditReport::new(0, dec!(0), dec!(0));
        let reporter = TerminalReporter;
        assert!(reporter.print_report(&report).is_ok());
    }

    #[test]
    fn test_terminal_report_with_findings() {
        let mut report = AuditReport::new(10, dec!(5000), dec!(5000));
        report.add_finding(AuditFinding {
            rule_name: "测试规则".to_string(),
            severity: Severity::Warning,
            message: "测试警告".to_string(),
            related_rows: vec![1, 2],
        });
        let reporter = TerminalReporter;
        assert!(reporter.print_report(&report).is_ok());
    }
}
