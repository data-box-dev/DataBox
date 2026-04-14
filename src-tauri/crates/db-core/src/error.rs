use thiserror::Error;

/// 整个项目统一的错误类型。
/// 各驱动 crate 将自己的错误通过 From impl 转换到这里。
#[derive(Debug, Error)]
pub enum DbError {
    // 连接类
    #[error("连接失败: {0}")]
    ConnectionFailed(String),

    #[error("连接不存在: {id}")]
    ConnectionNotFound { id: String },

    #[error("连接超时")]
    ConnectionTimeout,

    // 查询类
    #[error("查询执行失败: {0}")]
    QueryFailed(String),

    #[error("SQL 语法错误: {0}")]
    SyntaxError(String),

    #[error("查询超时（超过 {timeout_secs}s）")]
    QueryTimeout { timeout_secs: u64 },

    // 数据类
    #[error("类型转换失败: 列 `{column}` 无法转换为 {target_type}")]
    TypeConversion { column: String, target_type: String },

    #[error("列不存在: `{0}`")]
    ColumnNotFound(String),

    // 权限类
    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("认证失败")]
    AuthFailed,

    // 其他
    #[error("序列化失败: {0}")]
    Serialization(String),

    #[error("不支持的操作: {0}")]
    Unsupported(String),

    #[error("内部错误: {0}")]
    Internal(String),
}

/// 统一 Result 类型，所有 trait 方法都返回这个
pub type DbResult<T> = Result<T, DbError>;

impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        DbError::Serialization(e.to_string())
    }
}