//! 日期连续性检查规则
//!
//! 检测账目日期是否存在异常间断或时序错误。

use super::AuditRule;
use crate::models::{AuditFinding, Severity, Transaction};
use anyhow::Result;

/// 日期连续性检查
pub struct DateContinuityCheck {
    /// 允许的最大日期间隔（天数）
    max_gap_days: i64,
}

impl DateContinuityCheck {
    pub fn new(max_gap_days: i64) -> Self {
        Self { max_gap_days }
    }
}

impl Default for DateContinuityCheck {
    fn default() -> Self {
        Self { max_gap_days: 7 }
    }
}

impl AuditRule for DateContinuityCheck {
    fn name(&self) -> &str {
        "日期连续性检查"
    }

    fn description(&self) -> &str {
        "检测账目日期是否存在异常间断或时序错误"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();
        if transactions.len() < 2 {
            return Ok(findings);
        }

        // 按日期排序
        let mut sorted: Vec<&Transaction> = transactions.iter().collect();
        sorted.sort_by_key(|t| t.date);

        // 检测时序倒退（原始数据顺序中日期倒退）
        for i in 1..transactions.len() {
            if transactions[i].date < transactions[i - 1].date {
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "第 {} 行日期 {} 早于第 {} 行日期 {}，存在时序倒退",
                        transactions[i].row_index, transactions[i].date,
                        transactions[i - 1].row_index, transactions[i - 1].date
                    ),
                    related_rows: vec![transactions[i - 1].row_index, transactions[i].row_index],
                });
            }
        }

        // 检测日期间断
        for i in 1..sorted.len() {
            let gap = (sorted[i].date - sorted[i - 1].date).num_days();
            if gap > self.max_gap_days {
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "日期间断：{} 到 {} 间隔 {} 天（阈值 {} 天）",
                        sorted[i - 1].date, sorted[i].date, gap, self.max_gap_days
                    ),
                    related_rows: vec![sorted[i - 1].row_index, sorted[i].row_index],
                });
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

    fn make_txn(row: usize, date: &str) -> Transaction {
        Transaction {
            row_index: row,
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            voucher_id: format!("V{:03}", row),
            account_code: "1001".to_string(),
            account_name: "测试".to_string(),
            description: "测试".to_string(),
            debit: dec!(100),
            credit: dec!(0),
        }
    }

    #[test]
    fn test_date_gap() {
        let checker = DateContinuityCheck::new(7);
        let txns = vec![
            make_txn(1, "2024-01-01"),
            make_txn(2, "2024-01-20"), // 间隔19天
        ];
        let findings = checker.check(&txns).unwrap();
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_no_gap() {
        let checker = DateContinuityCheck::new(7);
        let txns = vec![
            make_txn(1, "2024-01-01"),
            make_txn(2, "2024-01-05"),
        ];
        let findings = checker.check(&txns).unwrap();
        assert!(findings.is_empty());
    }
}
