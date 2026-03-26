//! CSV 格式报告导出
//!
//! 将审计发现导出为 CSV 格式，便于在 Excel 等工具中查看和筛选。

use super::ReportExport;
use crate::models::AuditReport;
use anyhow::{Context, Result};
use std::path::Path;

/// CSV 导出器
pub struct CsvExporter;

impl ReportExport for CsvExporter {
    fn export(&self, report: &AuditReport, path: &Path) -> Result<()> {
        let mut writer = csv::Writer::from_path(path)
            .with_context(|| format!("创建 CSV 文件失败: {}", path.display()))?;

        // 写入表头
        writer.write_record(["序号", "严重级别", "规则名称", "说明", "涉及行"])?;

        // 写入每条发现
        for (i, finding) in report.findings.iter().enumerate() {
            let rows_str: Vec<String> = finding.related_rows.iter().map(|r| r.to_string()).collect();
            writer.write_record([
                (i + 1).to_string(),
                finding.severity.to_string(),
                finding.rule_name.clone(),
                finding.message.clone(),
                rows_str.join(";"),
            ])?;
        }

        writer.flush()?;
        println!("审计报告已导出至: {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuditFinding, AuditReport, Severity};
    use rust_decimal_macros::dec;
    use tempfile::NamedTempFile;

    #[test]
    fn test_csv_export() {
        let mut report = AuditReport::new(5, dec!(10000), dec!(10000));
        report.add_finding(AuditFinding {
            rule_name: "测试规则".to_string(),
            severity: Severity::Error,
            message: "错误描述".to_string(),
            related_rows: vec![1, 3],
        });

        let file = NamedTempFile::with_suffix(".csv").unwrap();
        let exporter = CsvExporter;
        assert!(exporter.export(&report, file.path()).is_ok());

        // 验证文件可被读取
        let content = std::fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("测试规则"));
        assert!(content.contains("1;3"));
    }
}
