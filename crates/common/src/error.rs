use thiserror::Error;

/// 统一错误类型
#[derive(Debug, Error)]
pub enum NetToolError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("资源未找到: {0}")]
    NotFound(String),

    #[error("输入无效: {0}")]
    InvalidInput(String),

    #[error("地址分配失败: {0}")]
    AllocationFailed(String),

    #[error("内部错误: {0}")]
    Internal(String),
}
