//! 借贷平衡检查规则
//!
//! 按凭证号分组，校验每笔会计分录的借方总额与贷方总额是否相等。
//! 支持配置容差值以应对四舍五入误差。

use super::AuditRule;
use crate::models::{AuditFinding, BalanceCheckConfig, Severity, Transaction};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::str::FromStr;

/// 借贷平衡检查
pub struct BalanceCheck {
    /// 允许的借贷差额容差
    tolerance: Decimal,
}

impl BalanceCheck {
    /// 从配置创建借贷平衡检查规则
    pub fn from_config(config: &BalanceCheckConfig) -> Self {
        let tolerance = config
            .tolerance
            .as_deref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(dec!(0.01));
        Self { tolerance }
    }

    /// 使用默认配置创建
    pub fn new() -> Self {
        Self {
            tolerance: dec!(0.01),
        }
    }
}

impl Default for BalanceCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditRule for BalanceCheck {
    fn name(&self) -> &str {
        "借贷平衡检查"
    }

    fn description(&self) -> &str {
        "校验每笔凭证的借方总额与贷方总额是否相等"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();

        // 按凭证号分组
        let mut voucher_groups: HashMap<&str, Vec<&Transaction>> = HashMap::new();
        for txn in transactions {
            voucher_groups
                .entry(&txn.voucher_id)
                .or_default()
                .push(txn);
        }

        for (voucher_id, txns) in &voucher_groups {
            let total_debit: Decimal = txns.iter().map(|t| t.debit).sum();
            let total_credit: Decimal = txns.iter().map(|t| t.credit).sum();
            let diff = (total_debit - total_credit).abs();

            if diff > self.tolerance {
                let related_rows: Vec<usize> = txns.iter().map(|t| t.row_index).collect();
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "凭证 {} 借贷不平衡：借方合计 {}，贷方合计 {}，差额 {}",
                        voucher_id, total_debit, total_credit, diff
                    ),
                    related_rows,
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

    fn make_txn(row: usize, voucher: &str, debit: Decimal, credit: Decimal) -> Transaction {
        Transaction {
            row_index: row,
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            voucher_id: voucher.to_string(),
            account_code: "1001".to_string(),
            account_name: "库存现金".to_string(),
            description: "测试".to_string(),
            debit,
            credit,
        }
    }

    #[test]
    fn test_balanced_voucher() {
        let checker = BalanceCheck::new();
        let txns = vec![
            make_txn(1, "V001", dec!(1000), dec!(0)),
            make_txn(2, "V001", dec!(0), dec!(1000)),
        ];
        let findings = checker.check(&txns).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_unbalanced_voucher() {
        let checker = BalanceCheck::new();
        let txns = vec![
            make_txn(1, "V001", dec!(1000), dec!(0)),
            make_txn(2, "V001", dec!(0), dec!(500)),
        ];
        let findings = checker.check(&txns).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn test_within_tolerance() {
        let checker = BalanceCheck::new();
        let txns = vec![
            make_txn(1, "V001", dec!(1000.00), dec!(0)),
            make_txn(2, "V001", dec!(0), dec!(999.995)),
        ];
        let findings = checker.check(&txns).unwrap();
        // 差额 0.005 < 容差 0.01，应通过
        assert!(findings.is_empty());
    }
}
