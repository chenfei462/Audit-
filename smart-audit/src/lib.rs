//! AuditLens v0.3 — 会计智能查账系统

pub mod ai_analysis;
pub mod anomaly;
pub mod auth;
pub mod cli;
pub mod data_parser;
pub mod database;
pub mod encryption;
pub mod models;
pub mod pdf_report;
pub mod report_gen;
pub mod rule_engine;
pub mod web;

use anyhow::{Context, Result};
use models::{AuditConfig, AuditReport};
use rust_decimal::Decimal;
use std::path::Path;

pub fn load_config(path: Option<&Path>) -> Result<AuditConfig> {
    let content = match path {
        Some(p) => std::fs::read_to_string(p)
            .with_context(|| format!("无法读取配置文件: {}", p.display()))?,
        None => include_str!("../config/default_rules.toml").to_string(),
    };
    let config: AuditConfig = toml::from_str(&content).context("解析配置文件失败")?;
    Ok(config)
}

pub fn run_audit(transactions: &[models::Transaction], config: &AuditConfig) -> Result<AuditReport> {
    let total_debit: Decimal = transactions.iter().map(|t| t.debit).sum();
    let total_credit: Decimal = transactions.iter().map(|t| t.credit).sum();
    let mut report = AuditReport::new(transactions.len(), total_debit, total_credit);

    let rules = rule_engine::create_rules_from_config(config);
    let executor = rule_engine::rules::RuleExecutor::new(rules);
    println!("已加载 {} 条审计规则", executor.rule_count());

    let rule_findings = executor.execute_all(transactions)?;
    for finding in rule_findings { report.add_finding(finding); }

    if let Some(ref anomaly_config) = config.anomaly {
        if anomaly_config.enabled {
            let detector = anomaly::ZScoreDetector::from_config(anomaly_config);
            let anomaly_findings = detector.detect(transactions)?;
            for finding in anomaly_findings { report.add_finding(finding); }
        }
    }
    Ok(report)
}
