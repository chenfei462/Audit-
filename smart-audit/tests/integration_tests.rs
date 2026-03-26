//! 集成测试
//!
//! 测试完整的审计流程：数据解析 → 规则检查 → 异常检测 → 报告生成

use auditlens::data_parser::csv_parser::CsvParser;
use auditlens::data_parser::DataSource;
use auditlens::models::Severity;
use std::path::Path;

/// 测试完整的审计流程
#[test]
fn test_full_audit_pipeline() {
    let csv_path = Path::new("tests/test_data/sample.csv");
    if !csv_path.exists() {
        panic!("测试数据文件不存在: {}", csv_path.display());
    }

    // 1. 解析数据
    let parser = CsvParser;
    let transactions = parser.parse(csv_path).expect("CSV 解析失败");
    assert!(!transactions.is_empty(), "交易记录不应为空");

    // 2. 加载默认配置
    let config = auditlens::load_config(None).expect("加载配置失败");

    // 3. 运行审计
    let report = auditlens::run_audit(&transactions, &config).expect("审计执行失败");

    // 4. 验证结果
    assert_eq!(report.total_transactions, transactions.len());
    // 测试数据中包含故意设计的问题，应有审计发现
    assert!(
        !report.findings.is_empty(),
        "测试数据中应有审计发现"
    );

    // V013 借贷不平衡（借方 10000，贷方 9000）
    let balance_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_name == "借贷平衡检查")
        .collect();
    assert!(
        !balance_errors.is_empty(),
        "应检测到借贷不平衡"
    );

    // V010 采购原材料 150000 > 阈值 50000
    let threshold_warnings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_name == "金额阈值检测")
        .collect();
    assert!(
        !threshold_warnings.is_empty(),
        "应检测到大额交易"
    );

    println!("集成测试通过！共发现 {} 条审计问题", report.findings.len());
}

/// 测试空数据集
#[test]
fn test_audit_empty_data() {
    let config = auditlens::load_config(None).expect("加载配置失败");
    let transactions = vec![];
    let report = auditlens::run_audit(&transactions, &config).expect("审计执行失败");
    assert_eq!(report.total_transactions, 0);
    assert!(report.findings.is_empty());
}

/// 测试报告严重级别统计
#[test]
fn test_report_severity_counts() {
    let csv_path = Path::new("tests/test_data/sample.csv");
    let parser = CsvParser;
    let transactions = parser.parse(csv_path).expect("CSV 解析失败");
    let config = auditlens::load_config(None).expect("加载配置失败");
    let report = auditlens::run_audit(&transactions, &config).expect("审计执行失败");

    let total = report.error_count() + report.warning_count() + report.info_count();
    assert_eq!(total, report.findings.len());
}

/// 测试 JSON 导出
#[test]
fn test_json_export_roundtrip() {
    use auditlens::report_gen::json_export::JsonExporter;
    use auditlens::report_gen::ReportExport;

    let csv_path = Path::new("tests/test_data/sample.csv");
    let parser = CsvParser;
    let transactions = parser.parse(csv_path).expect("CSV 解析失败");
    let config = auditlens::load_config(None).expect("加载配置失败");
    let report = auditlens::run_audit(&transactions, &config).expect("审计执行失败");

    let tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
    let exporter = JsonExporter;
    exporter.export(&report, tmp.path()).expect("JSON 导出失败");

    // 验证 JSON 可被解析
    let content = std::fs::read_to_string(tmp.path()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(value["total_transactions"], transactions.len());
}

/// 测试 CSV 导出
#[test]
fn test_csv_export() {
    use auditlens::report_gen::csv_export::CsvExporter;
    use auditlens::report_gen::ReportExport;

    let csv_path = Path::new("tests/test_data/sample.csv");
    let parser = CsvParser;
    let transactions = parser.parse(csv_path).expect("CSV 解析失败");
    let config = auditlens::load_config(None).expect("加载配置失败");
    let report = auditlens::run_audit(&transactions, &config).expect("审计执行失败");

    let tmp = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
    let exporter = CsvExporter;
    exporter.export(&report, tmp.path()).expect("CSV 导出失败");

    let content = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(content.contains("序号"));
    assert!(content.contains("严重级别"));
}
