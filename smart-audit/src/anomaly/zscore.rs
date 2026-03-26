//! Z-Score 异常检测
//!
//! 计算每笔交易金额相对于整体分布的偏离程度（Z-Score），
//! 将超过设定标准差倍数的交易标记为异常。
//! 支持按科目分组进行更精确的异常检测。

use crate::models::{AnomalyConfig, AuditFinding, Severity, Transaction};
use anyhow::Result;
use rayon::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;

/// Z-Score 异常检测器
pub struct ZScoreDetector {
    /// Z-Score 阈值（通常为 2.0 ~ 3.0）
    threshold: f64,
    /// 是否按科目分组检测
    group_by_account: bool,
}

impl ZScoreDetector {
    /// 从配置创建
    pub fn from_config(config: &AnomalyConfig) -> Self {
        Self {
            threshold: config.zscore_threshold.unwrap_or(3.0),
            group_by_account: config.group_by_account.unwrap_or(true),
        }
    }

    /// 使用默认参数创建
    pub fn new(threshold: f64, group_by_account: bool) -> Self {
        Self {
            threshold,
            group_by_account,
        }
    }

    /// 执行异常检测
    pub fn detect(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        if self.group_by_account {
            self.detect_by_group(transactions)
        } else {
            self.detect_global(transactions)
        }
    }

    /// 全局异常检测（不分组）
    fn detect_global(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let amounts: Vec<f64> = transactions
            .iter()
            .map(|t| {
                let net = t.debit - t.credit;
                net.abs().to_f64().unwrap_or(0.0)
            })
            .collect();

        let (mean, std_dev) = compute_stats(&amounts);
        if std_dev == 0.0 {
            return Ok(Vec::new());
        }

        let findings: Vec<AuditFinding> = transactions
            .par_iter()
            .enumerate()
            .filter_map(|(i, txn)| {
                let amount = amounts[i];
                let zscore = (amount - mean) / std_dev;

                if zscore.abs() > self.threshold {
                    Some(AuditFinding {
                        rule_name: "Z-Score 异常检测".to_string(),
                        severity: if zscore.abs() > self.threshold * 1.5 {
                            Severity::Error
                        } else {
                            Severity::Warning
                        },
                        message: format!(
                            "第 {} 行：异常金额 {}（Z-Score: {:.2}，均值: {:.2}，标准差: {:.2}），凭证 {}",
                            txn.row_index,
                            txn.debit.max(txn.credit),
                            zscore,
                            mean,
                            std_dev,
                            txn.voucher_id
                        ),
                        related_rows: vec![txn.row_index],
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(findings)
    }

    /// 按科目分组的异常检测
    fn detect_by_group(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        // 按科目分组
        let mut groups: HashMap<&str, Vec<&Transaction>> = HashMap::new();
        for txn in transactions {
            groups.entry(&txn.account_code).or_default().push(txn);
        }

        let mut all_findings = Vec::new();

        for (account_code, group) in &groups {
            // 每组至少需要 3 条记录才有统计意义
            if group.len() < 3 {
                continue;
            }

            let amounts: Vec<f64> = group
                .iter()
                .map(|t| {
                    let net = t.debit - t.credit;
                    net.abs().to_f64().unwrap_or(0.0)
                })
                .collect();

            let (mean, std_dev) = compute_stats(&amounts);
            if std_dev == 0.0 {
                continue;
            }

            for (i, txn) in group.iter().enumerate() {
                let zscore = (amounts[i] - mean) / std_dev;
                if zscore.abs() > self.threshold {
                    all_findings.push(AuditFinding {
                        rule_name: "Z-Score 异常检测（分组）".to_string(),
                        severity: if zscore.abs() > self.threshold * 1.5 {
                            Severity::Error
                        } else {
                            Severity::Warning
                        },
                        message: format!(
                            "第 {} 行：科目 {}({}) 内异常金额 {}（Z-Score: {:.2}），凭证 {}",
                            txn.row_index,
                            account_code,
                            txn.account_name,
                            txn.debit.max(txn.credit),
                            zscore,
                            txn.voucher_id
                        ),
                        related_rows: vec![txn.row_index],
                    });
                }
            }
        }

        Ok(all_findings)
    }
}

/// 计算均值和标准差
fn compute_stats(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    (mean, std_dev)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    fn make_txn(row: usize, account: &str, debit: rust_decimal::Decimal) -> Transaction {
        Transaction {
            row_index: row,
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            voucher_id: format!("V{:03}", row),
            account_code: account.to_string(),
            account_name: "测试".to_string(),
            description: "测试".to_string(),
            debit,
            credit: rust_decimal::Decimal::ZERO,
        }
    }

    #[test]
    fn test_compute_stats() {
        let values = vec![10.0, 20.0, 30.0];
        let (mean, std_dev) = compute_stats(&values);
        assert!((mean - 20.0).abs() < 0.001);
        assert!((std_dev - 8.165).abs() < 0.01);
    }

    #[test]
    fn test_detect_outlier() {
        let detector = ZScoreDetector::new(2.0, false);
        let txns = vec![
            make_txn(1, "1001", dec!(100)),
            make_txn(2, "1001", dec!(105)),
            make_txn(3, "1001", dec!(98)),
            make_txn(4, "1001", dec!(102)),
            make_txn(5, "1001", dec!(10000)), // 明显异常
        ];
        let findings = detector.detect(&txns).unwrap();
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.related_rows.contains(&5)));
    }

    #[test]
    fn test_no_anomaly() {
        let detector = ZScoreDetector::new(3.0, false);
        let txns = vec![
            make_txn(1, "1001", dec!(100)),
            make_txn(2, "1001", dec!(101)),
            make_txn(3, "1001", dec!(99)),
            make_txn(4, "1001", dec!(100)),
        ];
        let findings = detector.detect(&txns).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_empty_stats() {
        let (mean, std) = compute_stats(&[]);
        assert_eq!(mean, 0.0);
        assert_eq!(std, 0.0);
    }
}
