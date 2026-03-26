//! CSV 文件解析器
//!
//! 将 CSV 格式的财务数据解析为系统内部的 `Transaction` 结构。
//! 期望的 CSV 列顺序：日期,凭证号,科目编码,科目名称,摘要,借方金额,贷方金额

use super::DataSource;
use crate::models::Transaction;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;

/// CSV 解析器
pub struct CsvParser;

/// CSV 行的原始数据结构（用于 serde 反序列化）
#[derive(Debug, serde::Deserialize)]
struct CsvRecord {
    #[serde(rename = "日期", alias = "date")]
    date: String,
    #[serde(rename = "凭证号", alias = "voucher_id")]
    voucher_id: String,
    #[serde(rename = "科目编码", alias = "account_code")]
    account_code: String,
    #[serde(rename = "科目名称", alias = "account_name")]
    account_name: String,
    #[serde(rename = "摘要", alias = "description")]
    description: String,
    #[serde(rename = "借方金额", alias = "debit")]
    debit: String,
    #[serde(rename = "贷方金额", alias = "credit")]
    credit: String,
}

impl DataSource for CsvParser {
    fn parse(&self, path: &Path) -> Result<Vec<Transaction>> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_path(path)
            .with_context(|| format!("无法打开 CSV 文件: {}", path.display()))?;

        let mut transactions = Vec::new();

        for (index, result) in reader.deserialize().enumerate() {
            let record: CsvRecord = result
                .with_context(|| format!("解析第 {} 行数据失败", index + 2))?;

            let transaction = parse_record(record, index + 2)
                .with_context(|| format!("转换第 {} 行数据失败", index + 2))?;

            transactions.push(transaction);
        }

        Ok(transactions)
    }
}

/// 将 CSV 原始记录转换为 Transaction
fn parse_record(record: CsvRecord, row_index: usize) -> Result<Transaction> {
    // 尝试多种日期格式
    let date = parse_date(&record.date)
        .with_context(|| format!("日期格式错误: '{}'", record.date))?;

    // 解析金额，空值或无效值视为 0
    let debit = parse_amount(&record.debit)?;
    let credit = parse_amount(&record.credit)?;

    Ok(Transaction {
        row_index,
        date,
        voucher_id: record.voucher_id.trim().to_string(),
        account_code: record.account_code.trim().to_string(),
        account_name: record.account_name.trim().to_string(),
        description: record.description.trim().to_string(),
        debit,
        credit,
    })
}

/// 尝试多种格式解析日期字符串
fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    let formats = [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%Y年%m月%d日",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%Y.%m.%d",
    ];
    for fmt in &formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(date);
        }
    }
    anyhow::bail!("无法识别的日期格式: '{}'", s)
}

/// 解析金额字符串为 Decimal，空值返回 0
fn parse_amount(s: &str) -> Result<Decimal> {
    let s = s.trim();
    if s.is_empty() || s == "-" || s == "0" {
        return Ok(Decimal::ZERO);
    }
    // 移除千分位逗号
    let cleaned = s.replace(',', "");
    Decimal::from_str(&cleaned)
        .with_context(|| format!("金额格式错误: '{}'", s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_csv(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_parse_csv_basic() {
        let csv_content = "日期,凭证号,科目编码,科目名称,摘要,借方金额,贷方金额\n\
                           2024-01-15,V001,1001,库存现金,提取现金,5000.00,0.00\n\
                           2024-01-15,V001,1002,银行存款,提取现金,0.00,5000.00\n";
        let file = create_test_csv(csv_content);
        let parser = CsvParser;
        let txns = parser.parse(file.path()).unwrap();

        assert_eq!(txns.len(), 2);
        assert_eq!(txns[0].debit, dec!(5000.00));
        assert_eq!(txns[0].credit, dec!(0.00));
        assert_eq!(txns[1].debit, dec!(0.00));
        assert_eq!(txns[1].credit, dec!(5000.00));
    }

    #[test]
    fn test_parse_date_formats() {
        assert!(parse_date("2024-01-15").is_ok());
        assert!(parse_date("2024/01/15").is_ok());
        assert!(parse_date("2024.01.15").is_ok());
        assert!(parse_date("invalid").is_err());
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("1000.50").unwrap(), dec!(1000.50));
        assert_eq!(parse_amount("1,000.50").unwrap(), dec!(1000.50));
        assert_eq!(parse_amount("").unwrap(), dec!(0));
        assert_eq!(parse_amount("-").unwrap(), dec!(0));
    }
}
