//! 重复交易识别规则
//!
//! 基于日期 + 金额 + 科目的组合，在指定时间窗口内检测疑似重复的交易记录。

use super::AuditRule;
use crate::models::{AuditFinding, DuplicateConfig, Severity, Transaction};
use anyhow::Result;
use std::collections::HashMap;

/// 重复交易识别
pub struct DuplicateCheck {
    /// 时间窗口（天数）
    time_window_days: i64,
}

impl DuplicateCheck {
    /// 从配置创建
    pub fn from_config(config: &DuplicateConfig) -> Self {
        Self {
            time_window_days: config.time_window_days.unwrap_or(1),
        }
    }

    /// 使用默认参数创建
    pub fn new(time_window_days: i64) -> Self {
        Self { time_window_days }
    }
}

/// 用于分组的交易特征键
#[derive(Hash, Eq, PartialEq)]
struct TxnKey {
    account_code: String,
    debit: String,
    credit: String,
}

impl AuditRule for DuplicateCheck {
    fn name(&self) -> &str {
        "重复交易识别"
    }

    fn description(&self) -> &str {
        "基于日期+金额+科目组合检测疑似重复记录"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();

        // 按科目+金额分组
        let mut groups: HashMap<TxnKey, Vec<&Transaction>> = HashMap::new();
        for txn in transactions {
            let key = TxnKey {
                account_code: txn.account_code.clone(),
                debit: txn.debit.to_string(),
                credit: txn.credit.to_string(),
            };
            groups.entry(key).or_default().push(txn);
        }

        // 在每个分组内检测时间窗口内的重复
        for (_, group) in &groups {
            if group.len() < 2 {
                continue;
            }

            let mut sorted = group.clone();
            sorted.sort_by_key(|t| t.date);

            for i in 1..sorted.len() {
                let days_diff = (sorted[i].date - sorted[i - 1].date).num_days();
                if days_diff <= self.time_window_days {
                    let related_rows = vec![sorted[i - 1].row_index, sorted[i].row_index];
                    findings.push(AuditFinding {
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "疑似重复交易：第 {} 行与第 {} 行（科目 {}，金额 借:{}/贷:{}，日期间隔 {} 天）",
                            sorted[i - 1].row_index,
                            sorted[i].row_index,
                            sorted[i].account_code,
                            sorted[i].debit,
                            sorted[i].credit,
                            days_diff
                        ),
                        related_rows,
                    });
                }
            }
        }

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn make_txn(row: usize, date: &str, account: &str, debit: rust_decimal::Decimal) -> Transaction {
        Transaction {
            row_index: row,
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            voucher_id: format!("V{:03}", row),
            account_code: account.to_string(),
            account_name: "测试科目".to_string(),
            description: "测试".to_string(),
            debit,
            credit: rust_decimal::Decimal::ZERO,
        }
    }

    #[test]
    fn test_no_duplicates() {
        let checker = DuplicateCheck::new(1);
        let txns = vec![
            make_txn(1, "2024-01-01", "1001", dec!(1000)),
            make_txn(2, "2024-01-15", "1001", dec!(1000)),
        ];
        let findings = checker.check(&txns).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_duplicate_detected() {
        let checker = DuplicateCheck::new(1);
        let txns = vec![
            make_txn(1, "2024-01-01", "1001", dec!(1000)),
            make_txn(2, "2024-01-01", "1001", dec!(1000)),
        ];
        let findings = checker.check(&txns).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }

    #[test]
    fn test_different_accounts_no_duplicate() {
        let checker = DuplicateCheck::new(1);
        let txns = vec![
            make_txn(1, "2024-01-01", "1001", dec!(1000)),
            make_txn(2, "2024-01-01", "1002", dec!(1000)),
        ];
        let findings = checker.check(&txns).unwrap();
        assert!(findings.is_empty());
    }
}
