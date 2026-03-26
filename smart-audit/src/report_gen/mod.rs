//! 报告生成模块

pub mod csv_export;
pub mod json_export;
pub mod pdf_export;
pub mod terminal;

use crate::models::AuditReport;
use anyhow::Result;
use std::path::Path;

pub trait ReportOutput {
    fn print_report(&self, report: &AuditReport) -> Result<()>;
}

pub trait ReportExport {
    fn export(&self, report: &AuditReport, path: &Path) -> Result<()>;
}
