//! 关联交易检测规则
//!
//! 检测可能存在的关联方交易模式：
//! 1. 同一天内相同金额在不同科目间频繁流转（资金回流）
//! 2. 特定科目对之间的高频对手交易
//! 3. 期末集中大额交易（粉饰报表）
use chrono::Datelike;
use super::AuditRule;
use crate::models::{AuditFinding, Severity, Transaction};
use anyhow::Result;
use std::collections::HashMap;

/// 关联交易检测器
pub struct RelatedPartyCheck {
    /// 资金回流检测：同一天相同金额出现的最小次数
    min_round_trip_count: usize,
    /// 期末天数（月末 N 天内的大额交易需关注）
    period_end_days: u32,
    /// 期末集中交易的金额占比阈值
    period_end_ratio: f64,
}

impl RelatedPartyCheck {
    pub fn new() -> Self {
        Self {
            min_round_trip_count: 3,
            period_end_days: 5,
            period_end_ratio: 0.3,
        }
    }
}

impl Default for RelatedPartyCheck {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditRule for RelatedPartyCheck {
    fn name(&self) -> &str {
        "关联交易检测"
    }

    fn description(&self) -> &str {
        "检测资金回流、高频对手交易、期末集中交易等关联交易模式"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut findings = Vec::new();

        // ── 检测 1：资金回流（同日同金额多次出现）──
        self.check_round_trip(transactions, &mut findings);

        // ── 检测 2：期末集中大额交易 ──
        self.check_period_end(transactions, &mut findings);

        // ── 检测 3：高频科目对 ──
        self.check_frequent_pairs(transactions, &mut findings);

        Ok(findings)
    }
}

impl RelatedPartyCheck {
    /// 同日同金额多次出现 → 资金回流嫌疑
    fn check_round_trip(&self, transactions: &[Transaction], findings: &mut Vec<AuditFinding>) {
        // key: (日期, 金额字符串) → 交易列表
        let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();

        for txn in transactions {
            let amount = txn.debit.max(txn.credit);
            if amount.is_zero() {
                continue;
            }
            let key = (txn.date.to_string(), amount.to_string());
            groups.entry(key).or_default().push(txn.row_index);
        }

        for ((date, amount), rows) in &groups {
            if rows.len() >= self.min_round_trip_count {
                findings.push(AuditFinding {
                    rule_name: "关联交易-资金回流".to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "{}日金额 {} 出现 {} 次，可能存在资金回流",
                        date, amount, rows.len()
                    ),
                    related_rows: rows.clone(),
                });
            }
        }
    }

    /// 月末集中大额交易 → 粉饰报表嫌疑
    fn check_period_end(&self, transactions: &[Transaction], findings: &mut Vec<AuditFinding>) {
        use rust_decimal::prelude::ToPrimitive;

        let total_amount: f64 = transactions
            .iter()
            .map(|t| t.debit.max(t.credit).to_f64().unwrap_or(0.0))
            .sum();

        if total_amount == 0.0 {
            return;
        }

        // 找月末交易
        let period_end_amount: f64 = transactions
            .iter()
            .filter(|t| {
                let day = t.date.day();
                let last_day = last_day_of_month(t.date.year(), t.date.month());
                day > last_day.saturating_sub(self.period_end_days)
            })
            .map(|t| t.debit.max(t.credit).to_f64().unwrap_or(0.0))
            .sum();

        let ratio = period_end_amount / total_amount;
        if ratio > self.period_end_ratio {
            let period_end_rows: Vec<usize> = transactions
                .iter()
                .filter(|t| {
                    let day = t.date.day();
                    let last_day = last_day_of_month(t.date.year(), t.date.month());
                    day > last_day.saturating_sub(self.period_end_days)
                })
                .map(|t| t.row_index)
                .collect();

            findings.push(AuditFinding {
                rule_name: "关联交易-期末集中".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "月末 {} 天内交易金额占比 {:.1}%（阈值 {:.1}%），可能存在期末粉饰",
                    self.period_end_days,
                    ratio * 100.0,
                    self.period_end_ratio * 100.0
                ),
                related_rows: period_end_rows,
            });
        }
    }

    /// 高频科目对检测
    fn check_frequent_pairs(&self, transactions: &[Transaction], findings: &mut Vec<AuditFinding>) {
        // 按凭证号分组，找出每张凭证涉及的科目对
        let mut voucher_groups: HashMap<&str, Vec<&Transaction>> = HashMap::new();
        for txn in transactions {
            voucher_groups.entry(&txn.voucher_id).or_default().push(txn);
        }

        // 统计科目对出现频率
        let mut pair_counts: HashMap<(String, String), usize> = HashMap::new();
        for (_voucher, txns) in &voucher_groups {
            let debits: Vec<&str> = txns.iter().filter(|t| t.is_debit()).map(|t| t.account_code.as_str()).collect();
            let credits: Vec<&str> = txns.iter().filter(|t| t.is_credit()).map(|t| t.account_code.as_str()).collect();

            for d in &debits {
                for c in &credits {
                    if d != c {
                        let key = (d.to_string(), c.to_string());
                        *pair_counts.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }

        // 找高频科目对（超过 5 次）
        for ((debit_acc, credit_acc), count) in &pair_counts {
            if *count >= 5 {
                findings.push(AuditFinding {
                    rule_name: "关联交易-高频科目对".to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "科目 {} → {} 出现 {} 次交易，请关注是否存在关联方交易",
                        debit_acc, credit_acc, count
                    ),
                    related_rows: vec![],
                });
            }
        }
    }
}

/// 获取某月的最后一天
fn last_day_of_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    #[test]
    fn test_last_day_of_month() {
        assert_eq!(last_day_of_month(2024, 1), 31);
        assert_eq!(last_day_of_month(2024, 2), 29); // 闰年
        assert_eq!(last_day_of_month(2023, 2), 28);
        assert_eq!(last_day_of_month(2024, 4), 30);
    }

    #[test]
    fn test_round_trip_detection() {
        let checker = RelatedPartyCheck { min_round_trip_count: 2, ..Default::default() };
        let txns: Vec<Transaction> = (0..3)
            .map(|i| Transaction {
                row_index: i + 1,
                date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
                voucher_id: format!("V{:03}", i + 1),
                account_code: "1001".to_string(),
                account_name: "测试".to_string(),
                description: "测试".to_string(),
                debit: dec!(5000),
                credit: dec!(0),
            })
            .collect();
        let findings = checker.check(&txns).unwrap();
        assert!(findings.iter().any(|f| f.rule_name.contains("资金回流")));
    }
}
