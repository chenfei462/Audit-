//! 数据解析模块
//!
//! 定义了 `DataSource` trait 作为统一的数据源接口，
//! 以及 CSV 和 Excel 两种格式的具体实现。

pub mod csv_parser;
pub mod excel_parser;

use crate::models::Transaction;
use anyhow::Result;
use std::path::Path;

/// 数据源 trait — 所有数据格式解析器的统一接口
///
/// 实现此 trait 以支持新的数据格式导入。
pub trait DataSource {
    /// 从文件路径解析交易记录
    ///
    /// # Arguments
    /// * `path` - 输入文件路径
    ///
    /// # Returns
    /// 解析后的交易记录列表
    fn parse(&self, path: &Path) -> Result<Vec<Transaction>>;
}
