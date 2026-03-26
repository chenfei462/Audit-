//! AI 智能分析模块
//!
//! 将审计结果发送给大语言模型（支持 OpenAI / DeepSeek / 通义千问等），
//! 获取智能化的审计建议和风险分析。

use crate::models::{AuditReport, Severity};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// AI 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// API 端点（支持 OpenAI 兼容接口）
    /// 例如：
    /// - OpenAI: https://api.openai.com/v1/chat/completions
    /// - DeepSeek: https://api.deepseek.com/v1/chat/completions
    /// - 通义千问: https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions
    /// - 本地 Ollama: http://localhost:11434/v1/chat/completions
    pub api_url: String,
    /// API 密钥
    pub api_key: String,
    /// 模型名称
    pub model: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.deepseek.com/v1/chat/completions".to_string(),
            api_key: String::new(),
            model: "deepseek-chat".to_string(),
        }
    }
}

/// OpenAI 兼容的请求格式
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// OpenAI 兼容的响应格式
#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

/// 构建审计分析的 prompt
fn build_audit_prompt(report: &AuditReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let infos = report.info_count();

    let mut findings_text = String::new();
    for (i, f) in report.findings.iter().enumerate().take(30) {
        let severity = match f.severity {
            Severity::Error => "错误",
            Severity::Warning => "警告",
            Severity::Info => "信息",
        };
        findings_text.push_str(&format!(
            "{}. [{}] {} - {}\n",
            i + 1,
            severity,
            f.rule_name,
            f.message
        ));
    }
    if report.findings.len() > 30 {
        findings_text.push_str(&format!("... 还有 {} 条发现未列出\n", report.findings.len() - 30));
    }

    format!(
        r#"你是一名资深注册会计师和审计专家。请根据以下审计数据，提供专业的审计分析报告。

## 审计概览
- 审计时间：{}
- 交易总数：{}
- 借方总额：{}
- 贷方总额：{}
- 发现错误：{} 条
- 发现警告：{} 条
- 信息提示：{} 条

## 审计发现明细
{}

请从以下角度进行分析：

### 1. 整体风险评估
对该账目的整体风险等级进行评定（低/中/高/极高），并说明理由。

### 2. 重点问题分析
针对发现的错误和警告，分析可能的原因和潜在影响。

### 3. 具体审计建议
提供具体的、可操作的改进建议，按优先级排列。

### 4. 合规性意见
评估账目是否存在违反会计准则或法规的风险。

### 5. 后续审计计划
建议下一步需要重点关注的审计领域。

请用中文回答，语言专业但易于理解。"#,
        report.audit_time,
        report.total_transactions,
        report.total_debit,
        report.total_credit,
        errors,
        warnings,
        infos,
        findings_text
    )
}

/// 调用 AI 接口进行审计分析
pub async fn analyze_with_ai(report: &AuditReport, config: &AiConfig) -> Result<String> {
    if config.api_key.is_empty() {
        return Ok(generate_local_analysis(report));
    }

    let prompt = build_audit_prompt(report);

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: "你是一名资深注册会计师和审计专家，擅长财务数据分析和风险评估。".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ],
        temperature: 0.3,
        max_tokens: 2000,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&config.api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&request)
        .send()
        .await
        .context("AI API 请求失败")?;

    let chat_response: ChatResponse = response
        .json()
        .await
        .context("解析 AI 响应失败")?;

    if let Some(error) = chat_response.error {
        anyhow::bail!("AI API 错误: {}", error.message);
    }

    if let Some(choices) = chat_response.choices {
        if let Some(choice) = choices.first() {
            return Ok(choice.message.content.clone());
        }
    }

    anyhow::bail!("AI 未返回有效响应")
}

/// 本地分析（不需要 API Key，基于规则生成报告）
pub fn generate_local_analysis(report: &AuditReport) -> String {
    let errors = report.error_count();
    let warnings = report.warning_count();
    let total = report.findings.len();

    // 风险评级
    let risk_level = if errors >= 3 {
        "极高"
    } else if errors >= 1 {
        "高"
    } else if warnings >= 5 {
        "中"
    } else {
        "低"
    };

    let risk_color = match risk_level {
        "极高" => "🔴",
        "高" => "🟠",
        "中" => "🟡",
        _ => "🟢",
    };

    let mut analysis = format!(
        r#"# AuditLens 智能审计分析报告

## 1. 整体风险评估

{} **风险等级：{}**

本次审计共检查 {} 笔交易，发现 {} 条问题（{} 个错误、{} 个警告）。

"#,
        risk_color, risk_level, report.total_transactions, total, errors, warnings
    );

    // 借贷差异分析
    let diff = report.total_debit - report.total_credit;
    if !diff.is_zero() {
        analysis.push_str(&format!(
            "⚠️ **借贷总额不平衡**：借方总额 {} 与贷方总额 {} 存在差额 {}，需重点核查。\n\n",
            report.total_debit, report.total_credit, diff.abs()
        ));
    } else {
        analysis.push_str("✅ 借贷总额平衡，整体账务框架正常。\n\n");
    }

    // 分类统计
    analysis.push_str("## 2. 重点问题分析\n\n");

    let mut rule_summary: std::collections::HashMap<&str, (usize, usize, usize)> =
        std::collections::HashMap::new();
    for f in &report.findings {
        let entry = rule_summary.entry(&f.rule_name).or_insert((0, 0, 0));
        match f.severity {
            Severity::Error => entry.0 += 1,
            Severity::Warning => entry.1 += 1,
            Severity::Info => entry.2 += 1,
        }
    }

    for (rule, (e, w, i)) in &rule_summary {
        let icon = if *e > 0 { "🔴" } else if *w > 0 { "🟡" } else { "🔵" };
        analysis.push_str(&format!(
            "- {} **{}**：{} 个错误 / {} 个警告 / {} 个信息\n",
            icon, rule, e, w, i
        ));
    }

    // 建议
    analysis.push_str("\n## 3. 具体审计建议\n\n");

    let mut priority = 1;
    if errors > 0 {
        analysis.push_str(&format!(
            "**{}. [紧急]** 立即核查所有标记为「错误」的 {} 条记录，尤其关注借贷不平衡的凭证。\n\n",
            priority, errors
        ));
        priority += 1;
    }

    // 检查是否有 Benford 异常
    let has_benford = report.findings.iter().any(|f| f.rule_name.contains("Benford") && f.severity == Severity::Warning);
    if has_benford {
        analysis.push_str(&format!(
            "**{}. [重要]** Benford 定律检测发现异常，建议对金额首位数字分布异常的科目进行抽样审查。\n\n",
            priority
        ));
        priority += 1;
    }

    // 检查是否有关联交易
    let has_related = report.findings.iter().any(|f| f.rule_name.contains("关联交易"));
    if has_related {
        analysis.push_str(&format!(
            "**{}. [重要]** 发现关联交易迹象，建议核实相关方关系并检查交易定价的公允性。\n\n",
            priority
        ));
        priority += 1;
    }

    // 检查重复交易
    let has_duplicate = report.findings.iter().any(|f| f.rule_name.contains("重复"));
    if has_duplicate {
        analysis.push_str(&format!(
            "**{}. [建议]** 存在疑似重复交易，请与业务部门确认是否为正常业务需要。\n\n",
            priority
        ));
        priority += 1;
    }

    analysis.push_str(&format!(
        "**{}. [常规]** 建议定期（每月/每季）使用 AuditLens 进行自动化审计，持续监控账务质量。\n\n",
        priority
    ));

    // 合规性
    analysis.push_str("## 4. 合规性意见\n\n");
    if errors > 0 {
        analysis.push_str("⚠️ 当前账务存在合规风险。借贷不平衡违反了《企业会计准则——基本准则》中复式记账的基本要求，建议在报表编制前完成纠正。\n\n");
    } else {
        analysis.push_str("✅ 未发现明显的会计准则违规问题，但建议对警告事项进行进一步核实。\n\n");
    }

    // 后续计划
    analysis.push_str("## 5. 后续审计计划\n\n");
    analysis.push_str("1. 对本次发现的错误和高风险警告进行逐项整改确认\n");
    analysis.push_str("2. 扩大抽样范围，对关联方交易进行专项审计\n");
    analysis.push_str("3. 下一审计周期重点关注大额交易和期末集中交易\n");
    analysis.push_str("4. 完善内部控制制度，减少人为错误\n\n");

    analysis.push_str("---\n*本报告由 AuditLens 智能审计系统自动生成*\n");

    analysis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AuditFinding, AuditReport, Severity};
    use rust_decimal_macros::dec;

    #[test]
    fn test_local_analysis() {
        let mut report = AuditReport::new(100, dec!(500000), dec!(495000));
        report.add_finding(AuditFinding {
            rule_name: "借贷平衡检查".to_string(),
            severity: Severity::Error,
            message: "凭证不平衡".to_string(),
            related_rows: vec![1],
        });
        report.add_finding(AuditFinding {
            rule_name: "金额阈值检测".to_string(),
            severity: Severity::Warning,
            message: "大额交易".to_string(),
            related_rows: vec![5],
        });

        let analysis = generate_local_analysis(&report);
        assert!(analysis.contains("风险等级"));
        assert!(analysis.contains("紧急"));
        assert!(analysis.contains("合规"));
    }
}
