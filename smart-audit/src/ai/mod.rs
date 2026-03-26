//! AI 智能分析模块
//!
//! 基于审计结果自动生成分析摘要和建议。
//! 支持接入 OpenAI / 通义千问 / DeepSeek 等大模型 API。
//! 如果未配置 API Key，则使用内置的规则生成本地分析报告。

use crate::models::{AuditReport, Severity};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// AI 分析配置
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// API 提供商: "openai", "deepseek", "qwen", "local"
    pub provider: String,
    /// API Key
    pub api_key: String,
    /// API 地址（可自定义）
    pub api_url: String,
    /// 模型名称
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "local".to_string(),
            api_key: String::new(),
            api_url: String::new(),
            model: String::new(),
        }
    }
}

/// AI 分析结果
#[derive(Debug, Serialize, Deserialize)]
pub struct AiAnalysis {
    /// 风险等级评估
    pub risk_level: String,
    /// 总体分析摘要
    pub summary: String,
    /// 具体问题分析
    pub issue_analysis: Vec<IssueDetail>,
    /// 审计建议
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueDetail {
    pub category: String,
    pub description: String,
    pub risk: String,
    pub suggestion: String,
}

/// 执行 AI 分析
pub async fn analyze(report: &AuditReport, config: &AiConfig) -> Result<AiAnalysis> {
    match config.provider.as_str() {
        "openai" | "deepseek" | "qwen" => {
            call_llm_api(report, config).await
        }
        _ => {
            // 本地规则分析（无需 API）
            Ok(local_analysis(report))
        }
    }
}

/// 调用大模型 API 进行分析
async fn call_llm_api(report: &AuditReport, config: &AiConfig) -> Result<AiAnalysis> {
    let prompt = build_prompt(report);

    let api_url = match config.provider.as_str() {
        "openai" => {
            if config.api_url.is_empty() {
                "https://api.openai.com/v1/chat/completions".to_string()
            } else {
                config.api_url.clone()
            }
        }
        "deepseek" => {
            if config.api_url.is_empty() {
                "https://api.deepseek.com/v1/chat/completions".to_string()
            } else {
                config.api_url.clone()
            }
        }
        "qwen" => {
            if config.api_url.is_empty() {
                "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".to_string()
            } else {
                config.api_url.clone()
            }
        }
        _ => config.api_url.clone(),
    };

    let model = if config.model.is_empty() {
        match config.provider.as_str() {
            "openai" => "gpt-4o-mini",
            "deepseek" => "deepseek-chat",
            "qwen" => "qwen-plus",
            _ => "gpt-4o-mini",
        }.to_string()
    } else {
        config.model.clone()
    };

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "你是一位专业的财务审计师。请根据审计系统的检查结果，用中文给出专业的分析报告。返回 JSON 格式，包含 risk_level(高/中/低)、summary(总结)、issue_analysis(数组，每项含 category/description/risk/suggestion)、recommendations(建议数组)。只返回 JSON，不要其他内容。"
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.3
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .context("AI API 请求失败")?;

    let response_json: serde_json::Value = response.json().await.context("AI API 响应解析失败")?;

    // 提取回复内容
    let content = response_json["choices"][0]["message"]["content"]
        .as_str()
        .context("AI 返回内容为空")?;

    // 清理可能的 markdown 包装
    let clean = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let analysis: AiAnalysis = serde_json::from_str(clean)
        .context("AI 返回的 JSON 格式不正确")?;

    Ok(analysis)
}

/// 构建发送给 AI 的提示词
fn build_prompt(report: &AuditReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();

    let mut findings_text = String::new();
    for (i, f) in report.findings.iter().take(30).enumerate() {
        findings_text.push_str(&format!(
            "{}. [{}] {}: {}\n",
            i + 1,
            match f.severity { Severity::Error => "错误", Severity::Warning => "警告", _ => "信息" },
            f.rule_name,
            f.message.chars().take(200).collect::<String>()
        ));
    }

    format!(
        "以下是一份自动化审计检查结果，请分析并给出专业意见：\n\n\
        审计概览：\n\
        - 交易总数: {}\n\
        - 借方总额: {}\n\
        - 贷方总额: {}\n\
        - 错误: {} 项\n\
        - 警告: {} 项\n\
        - 信息: {} 项\n\n\
        审计发现（前30条）：\n{}",
        report.total_transactions, report.total_debit, report.total_credit,
        errors, warnings, infos, findings_text
    )
}

/// 本地规则分析（无需 API，基于规则逻辑生成）
fn local_analysis(report: &AuditReport) -> AiAnalysis {
    let errors = report.error_count();
    let warnings = report.warning_count();

    // 风险等级
    let risk_level = if errors >= 3 {
        "高".to_string()
    } else if errors >= 1 || warnings >= 5 {
        "中".to_string()
    } else {
        "低".to_string()
    };

    // 分类统计
    let mut categories: std::collections::HashMap<String, (usize, usize, usize)> = std::collections::HashMap::new();
    for f in &report.findings {
        let cat = simplify_rule_name(&f.rule_name);
        let entry = categories.entry(cat).or_insert((0, 0, 0));
        match f.severity {
            Severity::Error => entry.0 += 1,
            Severity::Warning => entry.1 += 1,
            Severity::Info => entry.2 += 1,
        }
    }

    // 生成问题分析
    let mut issue_analysis = Vec::new();
    for (cat, (e, w, _i)) in &categories {
        if *e > 0 || *w > 0 {
            let (risk, suggestion) = match cat.as_str() {
                "借贷平衡" => ("高", "立即核查不平衡凭证，确认是否存在记账错误或资金挪用"),
                "金额阈值" => ("中", "对大额交易逐笔核实，确认业务真实性和审批流程"),
                "重复交易" => ("中", "核查重复交易是否为误操作，是否需要冲红处理"),
                "日期连续" => ("低", "确认日期间断是否正常（如节假日），排除漏记可能"),
                "整数金额" => ("中", "高频整数金额需关注是否存在虚构交易或预算凑数"),
                "Benford" => ("中", "首位数字分布异常可能暗示人为数据篡改，建议抽样核查"),
                "关联交易" => ("高", "关联方交易需确认定价公允性和信息披露合规性"),
                "Z-Score" => ("中", "统计异常交易需逐笔核实业务背景"),
                _ => ("低", "建议人工复核确认"),
            };
            issue_analysis.push(IssueDetail {
                category: cat.clone(),
                description: format!("发现 {} 项错误、{} 项警告", e, w),
                risk: risk.to_string(),
                suggestion: suggestion.to_string(),
            });
        }
    }

    // 排序：高风险在前
    issue_analysis.sort_by(|a, b| {
        let order = |r: &str| match r { "高" => 0, "中" => 1, _ => 2 };
        order(&a.risk).cmp(&order(&b.risk))
    });

    // 生成建议
    let mut recommendations = Vec::new();
    if errors > 0 {
        recommendations.push("优先处理所有标记为「错误」的审计发现，这些问题可能涉及合规风险".to_string());
    }
    if categories.contains_key("借贷平衡") {
        recommendations.push("对借贷不平衡的凭证进行逐笔核对，联系相关业务部门确认".to_string());
    }
    if categories.contains_key("关联交易") {
        recommendations.push("审查关联方交易的定价依据和审批流程，确保符合《企业会计准则》要求".to_string());
    }
    if categories.contains_key("Benford") {
        recommendations.push("对 Benford 定律检测异常的科目进行抽样审计，重点关注金额集中区间".to_string());
    }
    if warnings > 5 {
        recommendations.push("警告数量较多，建议增加审计抽样比例，扩大核查范围".to_string());
    }
    recommendations.push("建议定期（至少每季度）运行审计检查，建立持续监控机制".to_string());

    // 总结
    let summary = format!(
        "本次审计共检查 {} 笔交易（借方 {}，贷方 {}），发现 {} 项问题。\
        其中错误 {} 项、警告 {} 项、信息 {} 项。\
        整体风险等级评估为「{}」。{}",
        report.total_transactions, report.total_debit, report.total_credit,
        report.findings.len(), errors, warnings, report.info_count(),
        risk_level,
        if errors > 0 { "建议立即采取纠正措施。" } else if warnings > 3 { "建议尽快复核相关事项。" } else { "账务整体状况良好。" }
    );

    AiAnalysis {
        risk_level,
        summary,
        issue_analysis,
        recommendations,
    }
}

/// 简化规则名称用于分类
fn simplify_rule_name(name: &str) -> String {
    if name.contains("平衡") { "借贷平衡".to_string() }
    else if name.contains("阈值") { "金额阈值".to_string() }
    else if name.contains("重复") { "重复交易".to_string() }
    else if name.contains("日期") { "日期连续".to_string() }
    else if name.contains("整数") { "整数金额".to_string() }
    else if name.contains("Benford") { "Benford".to_string() }
    else if name.contains("关联") { "关联交易".to_string() }
    else if name.contains("Z-Score") { "Z-Score".to_string() }
    else { name.to_string() }
}
