//! 数据库模块
//!
//! 支持从 MySQL 和 PostgreSQL 数据库直接读取交易数据。

use crate::models::Transaction;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::Row;

/// 数据库类型
#[derive(Debug, Clone)]
pub enum DatabaseType {
    MySQL,
    PostgreSQL,
}

/// 数据库配置
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub connection_string: String,
    pub db_type: DatabaseType,
    pub query: Option<String>,
    pub table_name: Option<String>,
}

const DEFAULT_COLUMNS: &str = "date, voucher_id, account_code, account_name, description, debit, credit";

impl DatabaseConfig {
    pub fn get_query(&self) -> String {
        if let Some(ref q) = self.query {
            q.clone()
        } else {
            let table = self.table_name.as_deref().unwrap_or("transactions");
            format!("SELECT {} FROM {} ORDER BY date", DEFAULT_COLUMNS, table)
        }
    }
}

/// 从 MySQL 读取
pub async fn fetch_from_mysql(config: &DatabaseConfig) -> Result<Vec<Transaction>> {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .connect(&config.connection_string)
        .await
        .context("MySQL 连接失败")?;

    let query = config.get_query();
    let rows = sqlx::query(&query).fetch_all(&pool).await.context("MySQL 查询失败")?;

    let mut transactions = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        transactions.push(Transaction {
            row_index: index + 1,
            date: row.try_get::<NaiveDate, _>("date").with_context(|| format!("第 {} 行日期读取失败", index + 1))?,
            voucher_id: row.try_get::<String, _>("voucher_id").unwrap_or_default(),
            account_code: row.try_get::<String, _>("account_code").unwrap_or_default(),
            account_name: row.try_get::<String, _>("account_name").unwrap_or_default(),
            description: row.try_get::<String, _>("description").unwrap_or_default(),
            debit: row.try_get::<Decimal, _>("debit").unwrap_or(Decimal::ZERO),
            credit: row.try_get::<Decimal, _>("credit").unwrap_or(Decimal::ZERO),
        });
    }
    pool.close().await;
    Ok(transactions)
}

/// 从 PostgreSQL 读取
pub async fn fetch_from_postgres(config: &DatabaseConfig) -> Result<Vec<Transaction>> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.connection_string)
        .await
        .context("PostgreSQL 连接失败")?;

    let query = config.get_query();
    let rows = sqlx::query(&query).fetch_all(&pool).await.context("PostgreSQL 查询失败")?;

    let mut transactions = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        transactions.push(Transaction {
            row_index: index + 1,
            date: row.try_get::<NaiveDate, _>("date").with_context(|| format!("第 {} 行日期读取失败", index + 1))?,
            voucher_id: row.try_get::<String, _>("voucher_id").unwrap_or_default(),
            account_code: row.try_get::<String, _>("account_code").unwrap_or_default(),
            account_name: row.try_get::<String, _>("account_name").unwrap_or_default(),
            description: row.try_get::<String, _>("description").unwrap_or_default(),
            debit: row.try_get::<Decimal, _>("debit").unwrap_or(Decimal::ZERO),
            credit: row.try_get::<Decimal, _>("credit").unwrap_or(Decimal::ZERO),
        });
    }
    pool.close().await;
    Ok(transactions)
}

/// 自动选择数据库类型并获取数据
pub async fn fetch_transactions(config: &DatabaseConfig) -> Result<Vec<Transaction>> {
    match config.db_type {
        DatabaseType::MySQL => fetch_from_mysql(config).await,
        DatabaseType::PostgreSQL => fetch_from_postgres(config).await,
    }
}
