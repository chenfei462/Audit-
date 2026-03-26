//! 审计规则引擎模块
//!
//! 定义了 `AuditRule` trait 作为所有审计规则的统一接口，
//! 以及规则注册和批量执行机制。

pub mod balance_check;
pub mod benford;
pub mod date_continuity;
pub mod duplicate;
pub mod related_party;
pub mod round_amount;
pub mod rules;
pub mod threshold;

use crate::models::{AuditConfig, AuditFinding, Transaction};
use anyhow::Result;

/// 审计规则 trait — 所有审计规则的统一接口
pub trait AuditRule: Send + Sync {
    /// 规则名称
    fn name(&self) -> &str;
    /// 规则描述
    fn description(&self) -> &str;
    /// 执行审计检查
    fn check(&self, transactions: &[Transaction]) -> Result<Vec<AuditFinding>>;
}

/// 从配置创建所有已启用的审计规则
pub fn create_rules_from_config(config: &AuditConfig) -> Vec<Box<dyn AuditRule>> {
    let mut audit_rules: Vec<Box<dyn AuditRule>> = Vec::new();

    if let Some(ref rules_config) = config.rules {
        // 借贷平衡检查
        if let Some(ref bc) = rules_config.balance_check {
            if bc.enabled {
                audit_rules.push(Box::new(balance_check::BalanceCheck::from_config(bc)));
            }
        }
        // 金额阈值检测
        if let Some(ref th) = rules_config.threshold {
            if th.enabled {
                audit_rules.push(Box::new(threshold::ThresholdCheck::from_config(th)));
            }
        }
        // 重复交易识别
        if let Some(ref dup) = rules_config.duplicate {
            if dup.enabled {
                audit_rules.push(Box::new(duplicate::DuplicateCheck::from_config(dup)));
            }
        }
        // 日期连续性检查
        if let Some(ref dc) = rules_config.date_continuity {
            if dc.enabled {
                let max_gap = dc.max_gap_days.unwrap_or(7);
                audit_rules.push(Box::new(date_continuity::DateContinuityCheck::new(max_gap)));
            }
        }
        // 整数金额预警
        if let Some(ref ra) = rules_config.round_amount {
            if ra.enabled {
                let threshold = ra.count_threshold.unwrap_or(5);
                audit_rules.push(Box::new(round_amount::RoundAmountCheck::new(threshold)));
            }
        }
    }

    // Benford 定律（始终启用）
    audit_rules.push(Box::new(benford::BenfordCheck::default()));

    // 关联交易检测（始终启用）
    audit_rules.push(Box::new(related_party::RelatedPartyCheck::default()));

    audit_rules
}
