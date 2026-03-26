//! PDF 审计报告生成
//!
//! 生成一个包含审计概览、图表数据和发现明细的 HTML 文件，
//! 可直接在浏览器中打开并打印为 PDF。
//! 适用于实际工作中提交正式审计报告。

use crate::models::{AuditReport, Severity};
use anyhow::{Context, Result};
use std::path::Path;

/// PDF（HTML）报告导出器
pub struct PdfExporter;

impl PdfExporter {
    /// 生成可打印的 HTML 审计报告
    pub fn export(&self, report: &AuditReport, path: &Path) -> Result<()> {
        let errors = report.error_count();
        let warnings = report.warning_count();
        let infos = report.info_count();

        let conclusion = if errors > 0 {
            "❌ 审计发现严重问题，需立即核查整改"
        } else if warnings > 0 {
            "⚠️ 审计发现警告事项，建议进一步核实"
        } else {
            "✅ 审计通过，未发现重大异常"
        };

        let conclusion_color = if errors > 0 { "#c53030" } else if warnings > 0 { "#b7791f" } else { "#276749" };

        // 构建发现明细表格行
        let mut findings_rows = String::new();
        for (i, finding) in report.findings.iter().enumerate() {
            let (severity_label, severity_bg, severity_color) = match finding.severity {
                Severity::Error => ("错误", "#fed7d7", "#c53030"),
                Severity::Warning => ("警告", "#fefcbf", "#b7791f"),
                Severity::Info => ("信息", "#bee3f8", "#2b6cb0"),
            };
            let rows_str: Vec<String> = finding.related_rows.iter().map(|r| r.to_string()).collect();
            let bg = if i % 2 == 0 { "#ffffff" } else { "#f7fafc" };
            // 将消息中的换行转为 <br>
            let message_html = finding.message.replace('\n', "<br>");

            findings_rows.push_str(&format!(
                r#"<tr style="background:{}">
                    <td style="padding:8px; border-bottom:1px solid #e2e8f0; text-align:center">{}</td>
                    <td style="padding:8px; border-bottom:1px solid #e2e8f0; text-align:center">
                        <span style="background:{}; color:{}; padding:2px 8px; border-radius:10px; font-size:12px">{}</span>
                    </td>
                    <td style="padding:8px; border-bottom:1px solid #e2e8f0">{}</td>
                    <td style="padding:8px; border-bottom:1px solid #e2e8f0; font-size:13px">{}</td>
                    <td style="padding:8px; border-bottom:1px solid #e2e8f0; text-align:center; font-size:12px; color:#718096">{}</td>
                </tr>"#,
                bg, i + 1, severity_bg, severity_color, severity_label,
                finding.rule_name, message_html,
                if rows_str.is_empty() { "-".to_string() } else { rows_str.join(", ") }
            ));
        }

        let html = format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>AuditLens 审计报告</title>
    <style>
        @page {{ margin: 20mm; }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: "Microsoft YaHei", "SimSun", sans-serif; color: #1a202c; font-size: 14px; line-height: 1.6; }}
        .header {{ background: linear-gradient(135deg, #1a365d, #2b6cb0); color: white; padding: 32px; margin: -20px -20px 24px; }}
        .header h1 {{ font-size: 24px; margin-bottom: 4px; }}
        .header .sub {{ font-size: 13px; opacity: 0.8; }}
        .section {{ margin-bottom: 24px; }}
        .section h2 {{ color: #1a365d; font-size: 17px; padding-bottom: 6px; border-bottom: 2px solid #2b6cb0; margin-bottom: 12px; }}
        .stats {{ display: flex; gap: 16px; margin-bottom: 20px; flex-wrap: wrap; }}
        .stat-box {{ flex: 1; min-width: 120px; background: #f7fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 16px; text-align: center; }}
        .stat-box .value {{ font-size: 24px; font-weight: 700; }}
        .stat-box .label {{ font-size: 12px; color: #718096; }}
        .stat-box.error .value {{ color: #e53e3e; }}
        .stat-box.warning .value {{ color: #dd6b20; }}
        .stat-box.info .value {{ color: #2b6cb0; }}
        .conclusion {{ padding: 16px; border-radius: 8px; font-size: 16px; font-weight: 600; margin-bottom: 20px; }}
        table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
        th {{ background: #2c5282; color: white; padding: 10px; text-align: left; }}
        .footer {{ margin-top: 32px; padding-top: 16px; border-top: 1px solid #e2e8f0; color: #a0aec0; font-size: 11px; text-align: center; }}
        @media print {{
            .header {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
            th {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>🔍 AuditLens 审计报告</h1>
        <div class="sub">会计智能查账系统 · 自动生成</div>
    </div>

    <div class="section">
        <h2>📊 审计概览</h2>
        <div class="stats">
            <div class="stat-box"><div class="value">{total_txn}</div><div class="label">交易总数</div></div>
            <div class="stat-box error"><div class="value">{errors}</div><div class="label">错误</div></div>
            <div class="stat-box warning"><div class="value">{warnings}</div><div class="label">警告</div></div>
            <div class="stat-box info"><div class="value">{infos}</div><div class="label">信息</div></div>
        </div>
        <div class="stats">
            <div class="stat-box"><div class="value" style="font-size:18px">{total_debit}</div><div class="label">借方总额</div></div>
            <div class="stat-box"><div class="value" style="font-size:18px">{total_credit}</div><div class="label">贷方总额</div></div>
            <div class="stat-box"><div class="value" style="font-size:18px">{audit_time}</div><div class="label">审计时间</div></div>
        </div>
    </div>

    <div class="section">
        <h2>📋 审计结论</h2>
        <div class="conclusion" style="background: {conclusion_bg}; color: {conclusion_color}">
            {conclusion}
            <div style="font-size:13px; font-weight:normal; margin-top:4px; color:#4a5568">
                共发现 {total_findings} 项问题（错误 {errors} 项 / 警告 {warnings} 项 / 信息 {infos} 项）
            </div>
        </div>
    </div>

    <div class="section">
        <h2>📝 审计发现明细</h2>
        <table>
            <thead>
                <tr><th style="width:40px">#</th><th style="width:60px">级别</th><th style="width:140px">规则</th><th>说明</th><th style="width:80px">涉及行</th></tr>
            </thead>
            <tbody>
                {findings_rows}
            </tbody>
        </table>
    </div>

    <div class="footer">
        <p>本报告由 AuditLens 会计智能查账系统自动生成 · 报告时间: {audit_time}</p>
        <p>报告仅供参考，最终审计意见以注册会计师签署的正式审计报告为准</p>
    </div>
</body>
</html>"#,
            total_txn = report.total_transactions,
            errors = errors,
            warnings = warnings,
            infos = infos,
            total_debit = report.total_debit,
            total_credit = report.total_credit,
            audit_time = report.audit_time,
            conclusion = conclusion,
            conclusion_color = conclusion_color,
            conclusion_bg = if errors > 0 { "#fed7d7" } else if warnings > 0 { "#fefcbf" } else { "#c6f6d5" },
            total_findings = report.findings.len(),
            findings_rows = findings_rows,
        );

        std::fs::write(path, &html)
            .with_context(|| format!("写入报告文件失败: {}", path.display()))?;

        println!("📄 审计报告已生成: {}", path.display());
        println!("   提示: 在浏览器中打开后按 Ctrl+P 即可打印为 PDF");
        Ok(())
    }
}
