//! 用户认证模块
//!
//! 提供用户注册、登录、会话管理功能。
//! 使用 bcrypt 对密码进行安全哈希存储。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub display_name: String,
    pub role: UserRole,
    pub created_at: String,
}

/// 用户角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    Auditor,
    Viewer,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "管理员"),
            UserRole::Auditor => write!(f, "审计员"),
            UserRole::Viewer => write!(f, "查看者"),
        }
    }
}

/// 用户存储（内存存储，生产环境应使用数据库）
pub struct UserStore {
    pub users: Mutex<HashMap<String, User>>,
}

impl UserStore {
    /// 创建用户存储并添加默认管理员账户
    pub fn new() -> Self {
        let mut users = HashMap::new();
        // 默认管理员：admin / admin123
        let admin_hash = bcrypt::hash("admin123", bcrypt::DEFAULT_COST).unwrap();
        users.insert(
            "admin".to_string(),
            User {
                username: "admin".to_string(),
                password_hash: admin_hash,
                display_name: "系统管理员".to_string(),
                role: UserRole::Admin,
                created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        );
        // 默认审计员：auditor / audit123
        let auditor_hash = bcrypt::hash("audit123", bcrypt::DEFAULT_COST).unwrap();
        users.insert(
            "auditor".to_string(),
            User {
                username: "auditor".to_string(),
                password_hash: auditor_hash,
                display_name: "审计员".to_string(),
                role: UserRole::Auditor,
                created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        );
        Self {
            users: Mutex::new(users),
        }
    }

    /// 验证用户登录
    pub fn verify(&self, username: &str, password: &str) -> Option<User> {
        let users = self.users.lock().unwrap();
        if let Some(user) = users.get(username) {
            if bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
                return Some(user.clone());
            }
        }
        None
    }

    /// 注册新用户
    pub fn register(
        &self,
        username: &str,
        password: &str,
        display_name: &str,
        role: UserRole,
    ) -> Result<User, String> {
        let mut users = self.users.lock().unwrap();
        if users.contains_key(username) {
            return Err("用户名已存在".to_string());
        }
        if username.len() < 3 {
            return Err("用户名至少3个字符".to_string());
        }
        if password.len() < 6 {
            return Err("密码至少6个字符".to_string());
        }

        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| format!("密码加密失败: {}", e))?;

        let user = User {
            username: username.to_string(),
            password_hash,
            display_name: display_name.to_string(),
            role,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        users.insert(username.to_string(), user.clone());
        Ok(user)
    }
}

impl Default for UserStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_login() {
        let store = UserStore::new();
        // 默认管理员登录
        assert!(store.verify("admin", "admin123").is_some());
        assert!(store.verify("admin", "wrong").is_none());

        // 注册新用户
        let result = store.register("testuser", "test123456", "测试用户", UserRole::Viewer);
        assert!(result.is_ok());
        assert!(store.verify("testuser", "test123456").is_some());
    }

    #[test]
    fn test_duplicate_register() {
        let store = UserStore::new();
        let result = store.register("admin", "pass123", "重复", UserRole::Viewer);
        assert!(result.is_err());
    }
}
