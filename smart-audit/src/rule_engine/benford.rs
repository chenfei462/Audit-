//! Benford 定律检测规则
//!
//! Benford 定律（首位数字定律）指出：在自然产生的数据集中，
//! 首位数字为 1 的概率约为 30.1%，为 2 的约 17.6%，以此类推。
//! 虚假或人为捏造的财务数据往往不符合此规律。

use super::AuditRule;
use crate::models::{AuditFinding, Severity, Transaction};
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;

/// Benford 定律检测器
pub struct BenfordCheck {
    /// 允许的最大偏差（百分比），例如 5.0 表示允许偏差 5%
    max_deviation: f64,
}

impl BenfordCheck {
    pub fn new(max_deviation: f64) -> Self {
        Self { max_deviation }
    }
}

impl Default for BenfordCheck {
    fn default() -> Self {
        Self { max_deviation: 5.0 }
    }
}

/// Benford 定律中每个首位数字（1-9）的理论概率
fn benford_expected() -> [f64; 9] {
    [
        0.301, // 1
        0.176, // 2
        0.125, // 3
        0.097, // 4
        0.079, // 5
        0.067, // 6
        0.058, // 7
        0.051, // 8
        0.046, // 9
    ]
}

/// 提取金额的首位数字（1-9），忽略 0 和负号
fn first_digit(amount: f64) -> Option<usize> {
    let abs = amount.abs();
    if abs < 1.0 {
        return None; // 忽略小于 1 的金额
    }
    let s = format!("{:.0}", abs);
    let ch = s.chars().next()?;
    let d = ch.to_digit(10)? as usize;
    if d >= 1 && d <= 9 {
        Some(d)
    } else {
        None
    }
}

impl AuditRule for BenfordCheck {
    fn name(&self) -> &str {
        "Benford 定律检测"
    }

    fn description(&self) -> &str {
        "检查交易金额的首位数字分布是否符合 Benford 定律"
    }

    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut counts = [0usize; 9]; // counts[0] 代表首位为 1 的计数
        let mut total = 0usize;

        for txn in transactions {
            let amount = txn.debit.max(txn.credit).to_f64().unwrap_or(0.0);
            if let Some(d) = first_digit(amount) {
                counts[d - 1] += 1;
                total += 1;
            }
        }

        if total < 50 {
            // 数据量太少，Benford 定律不可靠
            return Ok(vec![AuditFinding {
                rule_name: self.name().to_string(),
                severity: Severity::Info,
                message: format!(
                    "有效交易仅 {} 笔（少于 50 笔），Benford 定律检测不可靠，已跳过",
                    total
                ),
                related_rows: vec![],
            }]);
        }

        let expected = benford_expected();
        let mut findings = Vec::new();
        let mut deviations = Vec::new();

        for i in 0..9 {
            let actual_pct = (counts[i] as f64 / total as f64) * 100.0;
            let expected_pct = expected[i] * 100.0;
            let deviation = (actual_pct - expected_pct).abs();
            deviations.push((i + 1, actual_pct, expected_pct, deviation));

            if deviation > self.max_deviation {
                findings.push(AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: if deviation > self.max_deviation * 2.0 {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                    message: format!(
                        "首位数字 {} 的出现频率 {:.1}%（理论值 {:.1}%），偏差 {:.1}% 超过阈值 {:.1}%",
                        i + 1, actual_pct, expected_pct, deviation, self.max_deviation
                    ),
                    related_rows: vec![],
                });
            }
        }

        // 添加总体分布概览
        let summary_lines: Vec<String> = deviations
            .iter()
            .map(|(d, a, e, dev)| format!("  数字{}: 实际{:.1}% / 理论{:.1}% (偏差{:.1}%)", d, a, e, dev))
            .collect();
        findings.push(AuditFinding {
            rule_name: self.name().to_string(),
            severity: Severity::Info,
            message: format!("Benford 分布概览（共 {} 笔）:\n{}", total, summary_lines.join("\n")),
            related_rows: vec![],
        });

        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_digit() {
        assert_eq!(first_digit(1234.0), Some(1));
        assert_eq!(first_digit(567.89), Some(5));
        assert_eq!(first_digit(0.5), None);
        assert_eq!(first_digit(9999.0), Some(9));
    }

    #[test]
    fn test_benford_expected_sums_to_one() {
        let sum: f64 = benford_expected().iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }
}
