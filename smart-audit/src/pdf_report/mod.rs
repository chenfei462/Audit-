//! PDF 审计报告生成模块
//!
//! 使用 printpdf 生成专业的 PDF 格式审计报告，
//! 包含审计概览、统计图表、发现明细等内容。

use crate::models::{AuditReport, Severity};
use anyhow::{Context, Result};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// PDF 报告生成器
pub struct PdfReportGenerator;

impl PdfReportGenerator {
    /// 生成 PDF 审计报告
    pub fn generate(report: &AuditReport, output_path: &Path) -> Result<()> {
        let (doc, page1, layer1) = PdfDocument::new(
            "AuditLens 审计报告",
            Mm(210.0),
            Mm(297.0),
            "主页",
        );

        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .context("加载字体失败")?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .context("加载粗体字体失败")?;

        let current_layer = doc.get_page(page1).get_layer(layer1);

        // ===== 标题 =====
        current_layer.use_text(
            "AuditLens Audit Report",
            24.0,
            Mm(20.0),
            Mm(270.0),
            &font_bold,
        );

        current_layer.use_text(
            "Smart Accounting Audit System",
            12.0,
            Mm(20.0),
            Mm(260.0),
            &font,
        );

        // 分隔线
        let line = Line {
            points: vec![
                (Point::new(Mm(20.0), Mm(255.0)), false),
                (Point::new(Mm(190.0), Mm(255.0)), false),
            ],
            is_closed: false,
        };
        current_layer.set_outline_color(Color::Rgb(Rgb::new(0.1, 0.2, 0.4, None)));
        current_layer.set_outline_thickness(1.5);
        current_layer.add_line(line);

        // ===== 审计概览 =====
        let mut y = 245.0;

        current_layer.use_text("AUDIT OVERVIEW", 14.0, Mm(20.0), Mm(y), &font_bold);
        y -= 10.0;

        let overview_items = vec![
            format!("Audit Time: {}", report.audit_time),
            format!("Total Transactions: {}", report.total_transactions),
            format!("Total Debit: {}", report.total_debit),
            format!("Total Credit: {}", report.total_credit),
            format!("Errors: {}", report.error_count()),
            format!("Warnings: {}", report.warning_count()),
            format!("Info: {}", report.info_count()),
        ];

        for item in &overview_items {
            current_layer.use_text(item, 10.0, Mm(25.0), Mm(y), &font);
            y -= 6.0;
        }

        // ===== 发现摘要 =====
        y -= 8.0;
        current_layer.use_text("FINDINGS SUMMARY", 14.0, Mm(20.0), Mm(y), &font_bold);
        y -= 10.0;

        // 表头
        current_layer.use_text("#", 9.0, Mm(20.0), Mm(y), &font_bold);
        current_layer.use_text("Severity", 9.0, Mm(30.0), Mm(y), &font_bold);
        current_layer.use_text("Rule", 9.0, Mm(55.0), Mm(y), &font_bold);
        current_layer.use_text("Description", 9.0, Mm(100.0), Mm(y), &font_bold);
        y -= 3.0;

        // 表头下划线
        let header_line = Line {
            points: vec![
                (Point::new(Mm(20.0), Mm(y)), false),
                (Point::new(Mm(190.0), Mm(y)), false),
            ],
            is_closed: false,
        };
        current_layer.set_outline_thickness(0.5);
        current_layer.add_line(header_line);
        y -= 5.0;

        let mut page_num = 1;
        let mut current_page = page1;
       let mut current_layer_ref = doc.get_page(current_page).get_layer(layer1);

        for (i, finding) in report.findings.iter().enumerate() {
            // 检查是否需要新页
            if y < 30.0 {
                page_num += 1;
                let (new_page, new_layer) = doc.add_page(
                    Mm(210.0),
                    Mm(297.0),
                    format!("Page {}", page_num),
                );
                current_page = new_page;
                current_layer_ref = doc.get_page(current_page).get_layer(new_layer);
                y = 280.0;

                // 页头
                current_layer_ref.use_text(
                    &format!("AuditLens Report - Page {}", page_num),
                    8.0,
                    Mm(20.0),
                    Mm(290.0),
                    &font,
                );
            }

            let severity_str = match finding.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARNING",
                Severity::Info => "INFO",
            };

            // 设置颜色
            let color = match finding.severity {
                Severity::Error => Color::Rgb(Rgb::new(0.8, 0.0, 0.0, None)),
                Severity::Warning => Color::Rgb(Rgb::new(0.8, 0.5, 0.0, None)),
                Severity::Info => Color::Rgb(Rgb::new(0.0, 0.3, 0.7, None)),
            };
            current_layer_ref.set_fill_color(color);

            current_layer_ref.use_text(
                &format!("{}", i + 1),
                8.0,
                Mm(20.0),
                Mm(y),
                &font,
            );
            current_layer_ref.use_text(severity_str, 8.0, Mm(30.0), Mm(y), &font_bold);

            // 恢复黑色
            current_layer_ref.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

            // 规则名（截断）
            let rule_name = if finding.rule_name.len() > 20 {
                format!("{}...", &finding.rule_name[..17])
            } else {
                finding.rule_name.clone()
            };
            current_layer_ref.use_text(&rule_name, 8.0, Mm(55.0), Mm(y), &font);

            // 描述（截断，只取 ASCII 安全部分）
            let desc = if finding.message.len() > 60 {
                let safe_msg: String = finding.message.chars().take(57).collect();
                format!("{}...", safe_msg)
            } else {
                finding.message.clone()
            };
            // 只输出 ASCII 字符（printpdf 内置字体不支持中文）
            let ascii_desc: String = desc.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect();
            current_layer_ref.use_text(&ascii_desc, 7.0, Mm(100.0), Mm(y), &font);

            y -= 6.0;
        }

        // ===== 页脚 =====
        y -= 10.0;
        if y > 20.0 {
            let footer_line = Line {
                points: vec![
                    (Point::new(Mm(20.0), Mm(y)), false),
                    (Point::new(Mm(190.0), Mm(y)), false),
                ],
                is_closed: false,
            };
            current_layer_ref.set_outline_color(Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None)));
            current_layer_ref.set_outline_thickness(0.5);
            current_layer_ref.add_line(footer_line);

            current_layer_ref.set_fill_color(Color::Rgb(Rgb::new(0.4, 0.4, 0.4, None)));
            current_layer_ref.use_text(
                "Generated by AuditLens v0.3 | Smart Accounting Audit System",
                7.0,
                Mm(20.0),
                Mm(y - 5.0),
                &font,
            );
        }

        // 保存 PDF
        let file = File::create(output_path)
            .with_context(|| format!("创建 PDF 文件失败: {}", output_path.display()))?;
        doc.save(&mut BufWriter::new(file))
            .context("保存 PDF 失败")?;

        println!("PDF 审计报告已生成: {}", output_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuditFinding, AuditReport, Severity};
    use rust_decimal_macros::dec;

    #[test]
    fn test_pdf_generation() {
        let mut report = AuditReport::new(10, dec!(50000), dec!(50000));
        report.add_finding(AuditFinding {
            rule_name: "Test Rule".to_string(),
            severity: Severity::Warning,
            message: "Test warning message".to_string(),
            related_rows: vec![1, 2],
        });

        let tmp = tempfile::NamedTempFile::with_suffix(".pdf").unwrap();
        let result = PdfReportGenerator::generate(&report, tmp.path());
        assert!(result.is_ok());
        assert!(tmp.path().exists());
    }
}
