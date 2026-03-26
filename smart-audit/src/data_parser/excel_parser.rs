//! Excel 文件解析器
//!
//! 使用 calamine 库将 Excel (.xlsx/.xls) 格式的财务数据
//! 解析为系统内部的 `Transaction` 结构。
//! 兼容多种日期格式和 ExcelDateTime 类型。

use super::DataSource;
use crate::models::Transaction;
use anyhow::{Context, Result};
use calamine::{open_workbook_auto, Data, Reader};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;

/// Excel 解析器
pub struct ExcelParser;

impl DataSource for ExcelParser {
    fn parse(&self, path: &Path) -> Result<Vec<Transaction>> {
        let mut workbook = open_workbook_auto(path)
            .with_context(|| format!("无法打开 Excel 文件: {}", path.display()))?;

        let sheet_names = workbook.sheet_names().to_owned();
        let sheet_name = sheet_names
            .first()
            .context("Excel 文件中没有工作表")?
            .clone();

        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("无法读取工作表: {}", sheet_name))?;

        let mut transactions = Vec::new();
        let mut rows = range.rows();

        let _header = rows.next().context("Excel 文件为空")?;

        for (index, row) in rows.enumerate() {
            if row.len() < 7 {
                continue;
            }
            let transaction = parse_excel_row(row, index + 2)
                .with_context(|| format!("解析第 {} 行数据失败", index + 2))?;
            transactions.push(transaction);
        }

        Ok(transactions)
    }
}

fn parse_excel_row(row: &[Data], row_index: usize) -> Result<Transaction> {
    let date = extract_date(&row[0])
        .with_context(|| format!("第 {} 行日期解析失败（值: {:?}）", row_index, &row[0]))?;
    let voucher_id = extract_string(&row[1]);
    let account_code = extract_string(&row[2]);
    let account_name = extract_string(&row[3]);
    let description = extract_string(&row[4]);
    let debit = extract_decimal(&row[5])
        .with_context(|| format!("第 {} 行借方金额解析失败", row_index))?;
    let credit = extract_decimal(&row[6])
        .with_context(|| format!("第 {} 行贷方金额解析失败", row_index))?;

    Ok(Transaction {
        row_index, date, voucher_id, account_code, account_name, description, debit, credit,
    })
}

/// 尝试将任意字符串解析为日期
fn try_parse_date_str(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    // 尝试 NaiveDateTime 格式
    let dt_formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
    ];
    for fmt in &dt_formats {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(ndt.date());
        }
    }
    // 尝试纯日期格式
    let date_formats = [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%Y年%m月%d日",
        "%Y.%m.%d",
        "%d/%m/%Y",
        "%m/%d/%Y",
    ];
    for fmt in &date_formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(date);
        }
    }
    anyhow::bail!("无法识别的日期格式: '{}'", s)
}

/// Excel 日期序列号转日期
fn excel_serial_to_date(serial: f64) -> Result<NaiveDate> {
    let days = serial as i64;
    if days < 1 || days > 2958465 {
        anyhow::bail!("Excel 日期序列号超出范围: {}", serial);
    }
    let base = NaiveDate::from_ymd_opt(1899, 12, 30)
        .context("内部日期计算错误")?;
    Ok(base + chrono::Duration::days(days))
}

fn extract_date(cell: &Data) -> Result<NaiveDate> {
    match cell {
        // ExcelDateTime 类型 → 先转字符串再解析
        Data::DateTime(ref dt) => {
            let s = format!("{}", dt);
            // 先尝试作为日期字符串解析
            if let Ok(date) = try_parse_date_str(&s) {
                return Ok(date);
            }
            // 如果格式不对，尝试作为数字解析（日期序列号）
            if let Ok(serial) = s.trim().parse::<f64>() {
                return excel_serial_to_date(serial);
            }
            anyhow::bail!("DateTime 转换失败: '{}'", s)
        }
        Data::DateTimeIso(ref s) => {
            try_parse_date_str(s)
        }
        Data::String(ref s) => {
            try_parse_date_str(s)
        }
        Data::Float(f) => {
            excel_serial_to_date(*f)
        }
        Data::Int(i) => {
            excel_serial_to_date(*i as f64)
        }
        Data::Empty => {
            anyhow::bail!("日期单元格为空")
        }
        _ => {
            // 最后的兜底：转成字符串再试
            let s = format!("{:?}", cell);
            try_parse_date_str(&s).or_else(|_| anyhow::bail!("不支持的日期单元格类型: {:?}", cell))
        }
    }
}

fn extract_string(cell: &Data) -> String {
    match cell {
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => {
            if *f == f.floor() { format!("{}", *f as i64) } else { format!("{}", f) }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(ref dt) => format!("{}", dt),
        Data::DateTimeIso(ref s) => s.clone(),
        _ => String::new(),
    }
}

fn extract_decimal(cell: &Data) -> Result<Decimal> {
    match cell {
        Data::Float(f) => {
            let s = format!("{:.2}", f);
            Decimal::from_str(&s).context("金额转换失败")
        }
        Data::Int(i) => Ok(Decimal::from(*i)),
        Data::String(s) => {
            let s = s.trim();
            if s.is_empty() || s == "-" {
                Ok(Decimal::ZERO)
            } else {
                Decimal::from_str(&s.replace(',', "")).context("金额格式错误")
            }
        }
        Data::Empty => Ok(Decimal::ZERO),
        _ => Ok(Decimal::ZERO),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_string() {
        assert_eq!(extract_string(&Data::String("hello".to_string())), "hello");
        assert_eq!(extract_string(&Data::Float(1001.0)), "1001");
        assert_eq!(extract_string(&Data::Int(42)), "42");
        assert_eq!(extract_string(&Data::Empty), "");
    }

    #[test]
    fn test_extract_decimal() {
        let result = extract_decimal(&Data::Float(1000.50)).unwrap();
        assert_eq!(result.to_string(), "1000.50");
        let result = extract_decimal(&Data::Empty).unwrap();
        assert_eq!(result, Decimal::ZERO);
        let result = extract_decimal(&Data::String("1,234.56".to_string())).unwrap();
        assert_eq!(result.to_string(), "1234.56");
    }

    #[test]
    fn test_try_parse_date_str() {
        assert!(try_parse_date_str("2024-01-15").is_ok());
        assert!(try_parse_date_str("2024/01/15").is_ok());
        assert!(try_parse_date_str("2024-01-15 10:30:00").is_ok());
        assert!(try_parse_date_str("invalid").is_err());
    }

    #[test]
    fn test_excel_serial_to_date() {
        // 2024-01-01 = Excel 序列号 45292
        let date = excel_serial_to_date(45292.0).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
    }
}
