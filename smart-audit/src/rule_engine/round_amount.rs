//! 整数金额预警规则
//!
//! 频繁出现的整数金额交易可能暗示虚构交易。

use super::AuditRule;
use crate::models::{AuditFinding, Severity, Transaction};
use anyhow::Result;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// 整数金额预警
pub struct RoundAmountCheck {
    /// 整数金额出现次数阈值
    count_threshold: usize,
}

impl RoundAmountCheck {
    pub fn new(count_threshold: usize) -> Self {
        Self { count_threshold }
    }
}

impl Default for RoundAmountCheck {
    fn default() -> Self {
        Self { count_threshold: 5 }
    }
}

impl AuditRule for RoundAmountCheck {
    fn name(&self) -> &str {
        "整数金额预警"
    }

    fn description(&self) -> &str {
        "标记频繁出现的整数金额交易（可能为虚构交易）"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();

        // 统计整数金额出现次数
        let mut round_counts: HashMap<String, Vec<usize>> = HashMap::new();

        for txn in transactions {
            let amount = txn.debit.max(txn.credit);
            if amount.is_zero() {
                continue;
            }
            // 判断是否为整数（小数部分为 0）
            if amount.fract() == Decimal::ZERO && amount >= Decimal::from(100) {
                round_counts
                    .entry(amount.to_string())
                    .or_default()
                    .push(txn.row_index);
            }
        }

        for (amount, rows) in &round_counts {
            if rows.len() >= self.count_threshold {
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "整数金额 {} 出现 {} 次（阈值 {} 次），请核实是否为虚构交易",
                        amount, rows.len(), self.count_threshold
                    ),
                    related_rows: rows.clone(),
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

    #[test]
    fn test_round_amount_warning() {
        let checker = RoundAmountCheck::new(2);
        let txns: Vec<Transaction> = (0..3)
            .map(|i| Transaction {
                row_index: i + 1,
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                voucher_id: format!("V{:03}", i),
                account_code: "1001".to_string(),
                account_name: "测试".to_string(),
                description: "测试".to_string(),
                debit: dec!(5000),
                credit: dec!(0),
            })
            .collect();

        let findings = checker.check(&txns).unwrap();
        assert!(!findings.is_empty());
    }
}
