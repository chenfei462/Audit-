//! 金额阈值检测规则
//!
//! 标记超过预设阈值的大额交易，帮助审计人员关注高风险交易。

use super::AuditRule;
use crate::models::{AuditFinding, Severity, ThresholdConfig, Transaction};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;

/// 金额阈值检测
pub struct ThresholdCheck {
    /// 金额阈值
    threshold: Decimal,
}

impl ThresholdCheck {
    /// 从配置创建
    pub fn from_config(config: &ThresholdConfig) -> Self {
        let threshold = config
            .amount_threshold
            .as_deref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(dec!(50000));
        Self { threshold }
    }

    /// 使用默认阈值创建
    pub fn new(threshold: Decimal) -> Self {
        Self { threshold }
    }
}

impl AuditRule for ThresholdCheck {
    fn name(&self) -> &str {
        "金额阈值检测"
    }

    fn description(&self) -> &str {
        "标记超过预设阈值的大额交易"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();

        for txn in transactions {
            let max_amount = txn.debit.max(txn.credit);
            if max_amount > self.threshold {
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "第 {} 行：大额交易 {}（阈值 {}），凭证号 {}，科目 {} {}",
                        txn.row_index, max_amount, self.threshold,
                        txn.voucher_id, txn.account_code, txn.account_name
                    ),
                    related_rows: vec![txn.row_index],
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

    fn make_txn(row: usize, debit: Decimal, credit: Decimal) -> Transaction {
        Transaction {
            row_index: row,
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            voucher_id: "V001".to_string(),
            account_code: "1001".to_string(),
            account_name: "库存现金".to_string(),
            description: "测试".to_string(),
            debit,
            credit,
        }
    }

    #[test]
    fn test_below_threshold() {
        let checker = ThresholdCheck::new(dec!(50000));
        let txns = vec![make_txn(1, dec!(10000), dec!(0))];
        let findings = checker.check(&txns).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_above_threshold() {
        let checker = ThresholdCheck::new(dec!(50000));
        let txns = vec![make_txn(1, dec!(100000), dec!(0))];
        let findings = checker.check(&txns).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
    }
}
