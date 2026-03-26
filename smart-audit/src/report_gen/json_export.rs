//! JSON 格式报告导出
//!
//! 将审计报告序列化为 JSON 格式并写入文件，
//! 便于与其他系统集成或进行二次处理。

use super::ReportExport;
use crate::models::AuditReport;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// JSON 导出器
pub struct JsonExporter;

impl ReportExport for JsonExporter {
    fn export(&self, report: &AuditReport, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(report)
            .context("序列化审计报告为 JSON 失败")?;

        fs::write(path, &json)
            .with_context(|| format!("写入 JSON 文件失败: {}", path.display()))?;

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
    fn test_json_export() {
        let mut report = AuditReport::new(5, dec!(10000), dec!(10000));
        report.add_finding(AuditFinding {
            rule_name: "测试".to_string(),
            severity: Severity::Warning,
            message: "测试消息".to_string(),
            related_rows: vec![1],
        });

        let file = NamedTempFile::new().unwrap();
        let exporter = JsonExporter;
        assert!(exporter.export(&report, file.path()).is_ok());

        // 验证文件可被反序列化
        let content = fs::read_to_string(file.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["total_transactions"], 5);
    }
}
