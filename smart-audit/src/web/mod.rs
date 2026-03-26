//! Web 服务器模块 v0.3
//! 新增：用户登录、PDF导出、AI分析 API

use crate::ai_analysis::{self, AiConfig};
use crate::auth::{UserRole, UserStore};
use crate::data_parser::csv_parser::CsvParser;
use crate::data_parser::excel_parser::ExcelParser;
use crate::data_parser::DataSource;
use crate::database::{DatabaseConfig, DatabaseType};
use crate::models::{AuditReport, Transaction};
use crate::pdf_report::PdfReportGenerator;
use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;
use actix_files as fs;
use actix_multipart::Multipart;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use futures_util::StreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Arc;

#[derive(Serialize)]
struct DaySummary {
    date: String,
    total_debit: String,
    total_credit: String,
    count: usize,
}

#[derive(Serialize)]
struct WebAuditReport {
    #[serde(flatten)]
    report: AuditReport,
    transactions_summary: Vec<DaySummary>,
}

fn summarize_by_date(transactions: &[Transaction]) -> Vec<DaySummary> {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, (Decimal, Decimal, usize)> = BTreeMap::new();
    for txn in transactions {
        let key = txn.date.to_string();
        let entry = map.entry(key).or_insert((Decimal::ZERO, Decimal::ZERO, 0));
        entry.0 += txn.debit;
        entry.1 += txn.credit;
        entry.2 += 1;
    }
    map.into_iter()
        .map(|(date, (d, c, n))| DaySummary { date, total_debit: d.to_string(), total_credit: c.to_string(), count: n })
        .collect()
}

/// 登录请求
#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

/// 注册请求
#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    display_name: String,
}

/// 登录
async fn login(
    req: web::Json<LoginRequest>,
    session: Session,
    store: web::Data<Arc<UserStore>>,
) -> impl Responder {
    match store.verify(&req.username, &req.password) {
        Some(user) => {
            let _ = session.insert("username", &user.username);
            let _ = session.insert("display_name", &user.display_name);
            let _ = session.insert("role", format!("{}", user.role));
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "username": user.username,
                "display_name": user.display_name,
                "role": format!("{}", user.role)
            }))
        }
        None => HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "error": "用户名或密码错误"
        })),
    }
}

/// 注册
async fn register(
    req: web::Json<RegisterRequest>,
    store: web::Data<Arc<UserStore>>,
) -> impl Responder {
    match store.register(&req.username, &req.password, &req.display_name, UserRole::Auditor) {
        Ok(user) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "username": user.username,
            "display_name": user.display_name,
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "error": e
        })),
    }
}

/// 登出
async fn logout(session: Session) -> impl Responder {
    session.purge();
    HttpResponse::Ok().json(serde_json::json!({"success": true}))
}

/// 获取当前用户
async fn current_user(session: Session) -> impl Responder {
    let username = session.get::<String>("username").unwrap_or(None);
    match username {
        Some(name) => {
            let display_name = session.get::<String>("display_name").unwrap_or(None).unwrap_or_default();
            let role = session.get::<String>("role").unwrap_or(None).unwrap_or_default();
            HttpResponse::Ok().json(serde_json::json!({
                "logged_in": true,
                "username": name,
                "display_name": display_name,
                "role": role
            }))
        }
        None => HttpResponse::Ok().json(serde_json::json!({"logged_in": false})),
    }
}

/// 文件审计
async fn audit_file(mut payload: Multipart) -> impl Responder {
    let mut file_data = Vec::new();
    let mut file_name = String::new();
    while let Some(Ok(mut field)) = payload.next().await {
        if let Some(disposition) = field.content_disposition() {
            if let Some(name) = disposition.get_filename() {
                file_name = name.to_string();
            }
        }
        while let Some(Ok(chunk)) = field.next().await {
            file_data.extend_from_slice(&chunk);
        }
    }
    if file_data.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "未接收到文件"}));
    }

    let temp_dir = std::env::temp_dir();
    let file_id = uuid::Uuid::new_v4().to_string();
    let ext = if file_name.ends_with(".xlsx") || file_name.ends_with(".xls") { ".xlsx" } else { ".csv" };
    let temp_path = temp_dir.join(format!("auditlens_{}{}", file_id, ext));

    if let Ok(mut f) = std::fs::File::create(&temp_path) {
        let _ = f.write_all(&file_data);
    }

    let transactions = if ext == ".xlsx" {
        ExcelParser.parse(&temp_path)
    } else {
        CsvParser.parse(&temp_path)
    };
    let _ = std::fs::remove_file(&temp_path);

    match transactions {
        Ok(txns) => match run_audit_and_respond(&txns) {
            Ok(r) => HttpResponse::Ok().json(r),
            Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{}", e)})),
        },
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"error": format!("数据解析失败: {}", e)})),
    }
}

/// 数据库审计
#[derive(Deserialize)]
struct DbAuditRequest {
    db_type: String,
    host: String,
    user: String,
    password: String,
    database: String,
    table: Option<String>,
    query: Option<String>,
}

async fn audit_database(req: web::Json<DbAuditRequest>) -> impl Responder {
    let db_type = match req.db_type.as_str() {
        "mysql" => DatabaseType::MySQL,
        "postgres" => DatabaseType::PostgreSQL,
        _ => return HttpResponse::BadRequest().json(serde_json::json!({"error": "不支持的数据库类型"})),
    };
    let conn_str = match db_type {
        DatabaseType::MySQL => format!("mysql://{}:{}@{}/{}", req.user, req.password, req.host, req.database),
        DatabaseType::PostgreSQL => format!("postgres://{}:{}@{}/{}", req.user, req.password, req.host, req.database),
    };
    let config = DatabaseConfig {
        connection_string: conn_str, db_type,
        query: req.query.clone().filter(|s| !s.is_empty()),
        table_name: req.table.clone().filter(|s| !s.is_empty()),
    };
    match crate::database::fetch_transactions(&config).await {
        Ok(txns) => match run_audit_and_respond(&txns) {
            Ok(r) => HttpResponse::Ok().json(r),
            Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{}", e)})),
        },
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"error": format!("{}", e)})),
    }
}

/// AI 分析请求
#[derive(Deserialize)]
struct AiAnalysisRequest {
    report: AuditReport,
    api_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
}

async fn ai_analyze(req: web::Json<AiAnalysisRequest>) -> impl Responder {
    let config = AiConfig {
        api_url: req.api_url.clone().unwrap_or_else(|| "https://api.deepseek.com/v1/chat/completions".to_string()),
        api_key: req.api_key.clone().unwrap_or_default(),
        model: req.model.clone().unwrap_or_else(|| "deepseek-chat".to_string()),
    };

    match ai_analysis::analyze_with_ai(&req.report, &config).await {
        Ok(analysis) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "analysis": analysis
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": format!("{}", e)
        })),
    }
}

/// PDF 导出请求
#[derive(Deserialize)]
struct PdfExportRequest {
    report: AuditReport,
}

async fn export_pdf(req: web::Json<PdfExportRequest>) -> impl Responder {
    let temp_dir = std::env::temp_dir();
    let file_id = uuid::Uuid::new_v4().to_string();
    let pdf_path = temp_dir.join(format!("audit_report_{}.pdf", file_id));

    match PdfReportGenerator::generate(&req.report, &pdf_path) {
        Ok(_) => {
            match std::fs::read(&pdf_path) {
                Ok(data) => {
                    let _ = std::fs::remove_file(&pdf_path);
                    HttpResponse::Ok()
                        .content_type("application/pdf")
                        .insert_header(("Content-Disposition", "attachment; filename=\"audit_report.pdf\""))
                        .body(data)
                }
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{}", e)})),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("{}", e)})),
    }
}

fn run_audit_and_respond(transactions: &[Transaction]) -> Result<WebAuditReport> {
    let config = crate::load_config(None)?;
    let report = crate::run_audit(transactions, &config)?;
    let summary = summarize_by_date(transactions);
    Ok(WebAuditReport { report, transactions_summary: summary })
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

/// 加密导出
#[derive(Deserialize)]
struct EncryptRequest {
    data: String,
    password: String,
}

async fn encrypt_data(req: web::Json<EncryptRequest>) -> impl Responder {
    match crate::encryption::encrypt(&req.data, &req.password) {
        Ok(encrypted) => HttpResponse::Ok().json(serde_json::json!({"success": true, "encrypted": encrypted})),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"success": false, "error": format!("{}", e)})),
    }
}

/// 解密导入
#[derive(Deserialize)]
struct DecryptRequest {
    encrypted: crate::encryption::EncryptedData,
    password: String,
}

async fn decrypt_data(req: web::Json<DecryptRequest>) -> impl Responder {
    match crate::encryption::decrypt(&req.encrypted, &req.password) {
        Ok(data) => HttpResponse::Ok().json(serde_json::json!({"success": true, "data": data})),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({"success": false, "error": format!("{}", e)})),
    }
}

pub async fn start_server(host: &str, port: u16) -> Result<()> {
    let bind_addr = format!("{}:{}", host, port);
    let user_store = Arc::new(UserStore::new());
    let secret_key = Key::generate();

    println!("🌐 AuditLens v0.3 Web 服务器启动中...");
    println!("📡 访问地址: http://{}", bind_addr);
    println!("👤 默认账户: admin / admin123 或 auditor / audit123");
    println!("按 Ctrl+C 停止服务器");

    HttpServer::new(move || {
        App::new()
            .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
            .app_data(web::Data::new(user_store.clone()))
            .route("/api/health", web::get().to(health_check))
            .route("/api/auth/login", web::post().to(login))
            .route("/api/auth/register", web::post().to(register))
            .route("/api/auth/logout", web::post().to(logout))
            .route("/api/auth/me", web::get().to(current_user))
            .route("/api/audit/file", web::post().to(audit_file))
            .route("/api/audit/database", web::post().to(audit_database))
            .route("/api/ai/analyze", web::post().to(ai_analyze))
            .route("/api/export/pdf", web::post().to(export_pdf))
            .route("/api/encrypt", web::post().to(encrypt_data))
            .route("/api/decrypt", web::post().to(decrypt_data))
            .service(fs::Files::new("/", "./static").index_file("index.html"))
    })
    .bind(&bind_addr)?
    .run()
    .await?;
    Ok(())
}
