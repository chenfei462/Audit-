//! 核心数据模型定义
//!
//! 定义了系统中使用的所有核心数据结构，包括交易记录、审计结果、
//! 审计配置等。所有金额字段使用 `Decimal` 类型确保精度。

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 交易记录 — 系统内部统一的账目数据结构
///
/// 所有外部数据（CSV、Excel 等）经解析后转换为此结构。
/// 金额使用 `Decimal` 类型，避免浮点精度问题。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// 行号（原始数据中的位置，用于定位问题）
    pub row_index: usize,
    /// 交易日期
    pub date: NaiveDate,
    /// 凭证号
    pub voucher_id: String,
    /// 会计科目编码
    pub account_code: String,
    /// 会计科目名称
    pub account_name: String,
    /// 摘要说明
    pub description: String,
    /// 借方金额
    pub debit: Decimal,
    /// 贷方金额
    pub credit: Decimal,
}

impl Transaction {
    /// 获取交易净额（借方为正，贷方为负）
    pub fn net_amount(&self) -> Decimal {
        self.debit - self.credit
    }

    /// 判断是否为借方交易
    pub fn is_debit(&self) -> bool {
        self.debit > Decimal::ZERO
    }

    /// 判断是否为贷方交易
    pub fn is_credit(&self) -> bool {
        self.credit > Decimal::ZERO
    }
}

/// 审计严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// 信息提示
    Info,
    /// 警告
    Warning,
    /// 错误（严重问题）
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "信息"),
            Severity::Warning => write!(f, "警告"),
            Severity::Error => write!(f, "错误"),
        }
    }
}

/// 单条审计发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    /// 规则名称
    pub rule_name: String,
    /// 严重级别
    pub severity: Severity,
    /// 描述信息
    pub message: String,
    /// 涉及的交易行号列表
    pub related_rows: Vec<usize>,
}

/// 审计报告 — 汇总所有审计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    /// 审计发现列表
    pub findings: Vec<AuditFinding>,
    /// 审计的交易总数
    pub total_transactions: usize,
    /// 借方总额
    pub total_debit: Decimal,
    /// 贷方总额
    pub total_credit: Decimal,
    /// 审计时间
    pub audit_time: String,
}

impl AuditReport {
    /// 创建空的审计报告
    pub fn new(total_transactions: usize, total_debit: Decimal, total_credit: Decimal) -> Self {
        Self {
            findings: Vec::new(),
            total_transactions,
            total_debit,
            total_credit,
            audit_time: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    /// 添加审计发现
    pub fn add_finding(&mut self, finding: AuditFinding) {
        self.findings.push(finding);
    }

    /// 合并另一个报告的发现
    pub fn merge(&mut self, other: AuditReport) {
        self.findings.extend(other.findings);
    }

    /// 按严重级别统计发现数量
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == severity).count()
    }

    /// 获取错误数量
    pub fn error_count(&self) -> usize {
        self.count_by_severity(Severity::Error)
    }

    /// 获取警告数量
    pub fn warning_count(&self) -> usize {
        self.count_by_severity(Severity::Warning)
    }

    /// 获取信息数量
    pub fn info_count(&self) -> usize {
        self.count_by_severity(Severity::Info)
    }
}

/// 审计规则配置（从 TOML 反序列化）
#[derive(Debug, Clone, Deserialize)]
pub struct AuditConfig {
    pub general: Option<GeneralConfig>,
    pub rules: Option<RulesConfig>,
    pub anomaly: Option<AnomalyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralConfig {
    pub decimal_places: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RulesConfig {
    pub balance_check: Option<BalanceCheckConfig>,
    pub threshold: Option<ThresholdConfig>,
    pub duplicate: Option<DuplicateConfig>,
    pub date_continuity: Option<DateContinuityConfig>,
    pub account_compliance: Option<AccountComplianceConfig>,
    pub round_amount: Option<RoundAmountConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BalanceCheckConfig {
    pub enabled: bool,
    pub tolerance: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdConfig {
    pub enabled: bool,
    pub amount_threshold: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateConfig {
    pub enabled: bool,
    pub time_window_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DateContinuityConfig {
    pub enabled: bool,
    pub max_gap_days: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountComplianceConfig {
    pub enabled: bool,
    pub valid_accounts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoundAmountConfig {
    pub enabled: bool,
    pub count_threshold: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnomalyConfig {
    pub enabled: bool,
    pub zscore_threshold: Option<f64>,
    pub group_by_account: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_transaction_net_amount() {
        let txn = Transaction {
            row_index: 1,
            date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            voucher_id: "V001".to_string(),
            account_code: "1001".to_string(),
            account_name: "库存现金".to_string(),
            description: "测试交易".to_string(),
            debit: dec!(1000.00),
            credit: dec!(0.00),
        };
        assert_eq!(txn.net_amount(), dec!(1000.00));
        assert!(txn.is_debit());
        assert!(!txn.is_credit());
    }

    #[test]
    fn test_audit_report_counts() {
        let mut report = AuditReport::new(10, dec!(5000.00), dec!(5000.00));
        report.add_finding(AuditFinding {
            rule_name: "test".to_string(),
            severity: Severity::Error,
            message: "错误".to_string(),
            related_rows: vec![1],
        });
        report.add_finding(AuditFinding {
            rule_name: "test".to_string(),
            severity: Severity::Warning,
            message: "警告".to_string(),
            related_rows: vec![2],
        });
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.info_count(), 0);
    }
}
