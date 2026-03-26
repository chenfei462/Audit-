//! 异常检测模块
//!
//! 基于统计方法自动检测异常交易记录。

pub mod zscore;

pub use zscore::ZScoreDetector;
