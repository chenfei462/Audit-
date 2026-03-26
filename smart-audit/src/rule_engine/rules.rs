//! 规则执行器
//!
//! 负责批量运行所有已注册的审计规则，收集并汇总审计发现。

use super::AuditRule;
use crate::models::{AuditFinding, Transaction};
use anyhow::Result;

/// 规则执行器 — 管理和执行所有审计规则
pub struct RuleExecutor {
    rules: Vec<Box<dyn AuditRule>>,
}

impl RuleExecutor {
    /// 创建规则执行器
    pub fn new(rules: Vec<Box<dyn AuditRule>>) -> Self {
        Self { rules }
    }

    /// 获取已注册的规则数量
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// 执行所有规则并返回汇总的审计发现
    pub fn execute_all(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
        let mut all_findings = Vec::new();

        for rule in &self.rules {
            match rule.check(transactions) {
                Ok(findings) => {
                    all_findings.extend(findings);
                }
                Err(e) => {
                    eprintln!("规则 '{}' 执行失败: {}", rule.name(), e);
                }
            }
        }

        Ok(all_findings)
    }

    /// 获取所有规则的名称列表
    pub fn rule_names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuditFinding, Severity, Transaction};
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    /// 测试用的简单规则：标记所有借方金额大于 0 的交易
    struct TestRule;
    impl AuditRule for TestRule {
        fn name(&self) -> &str { "测试规则" }
        fn description(&self) -> &str { "测试用规则" }
        fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>> {
            let findings: Vec<_> = transactions.iter()
                .filter(|t| t.debit > dec!(0))
                .map(|t| AuditFinding {
                    rule_name: self.name().to_string(),
                    severity: Severity::Info,
                    message: format!("借方交易: {}", t.debit),
                    related_rows: vec![t.row_index],
                })
                .collect();
            Ok(findings)
        }
    }

    #[test]
    fn test_rule_executor() {
        let rules: Vec<Box<dyn AuditRule>> = vec![Box::new(TestRule)];
        let executor = RuleExecutor::new(rules);

        let txns = vec![
            Transaction {
                row_index: 1,
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                voucher_id: "V001".to_string(),
                account_code: "1001".to_string(),
                account_name: "库存现金".to_string(),
                description: "test".to_string(),
                debit: dec!(100),
                credit: dec!(0),
            },
        ];

        let findings = executor.execute_all(&txns).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(executor.rule_count(), 1);
    }
}
