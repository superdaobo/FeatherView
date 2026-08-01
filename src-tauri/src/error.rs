use serde::ser::{Serialize, SerializeStruct, Serializer};

/// 统一的用户可读错误类型。
///
/// 所有错误通过 `user_message` 转换为用户可理解的中文说明，
/// 底层系统错误细节仅写入日志，不直接暴露给普通用户。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("文件不存在")]
    NotFound(String),
    #[error("没有权限读取该文件")]
    PermissionDenied(String),
    #[error("文件体积过大")]
    TooLarge(String),
    #[error("该文件不是文本文件，无法以文本方式阅读")]
    // 预留：前端试图将二进制文件作为文本处理时使用
    #[allow(dead_code)]
    BinaryFile,
    #[error("无法识别文件编码")]
    UnknownEncoding,
    #[error("文件在读取过程中被修改")]
    // 大文件会话 read_range 检测到文件变化（夜间合同 §2 / §7）
    FileChanged(String),
    #[error("读取会话不存在或已关闭")]
    SessionNotFound,
    #[error("读取文件失败")]
    Io(String),
    #[error("内部错误")]
    // 预留：未来内部逻辑错误
    #[allow(dead_code)]
    Internal(String),
}

impl AppError {
    /// 展示给用户的简短错误标题。
    pub fn title(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "文件不存在",
            AppError::PermissionDenied(_) => "无法访问文件",
            AppError::TooLarge(_) => "文件体积过大",
            AppError::BinaryFile => "不支持的文本格式",
            AppError::UnknownEncoding => "编码识别失败",
            AppError::FileChanged(_) => "文件已变更",
            AppError::SessionNotFound => "读取会话失效",
            AppError::Io(_) => "读取文件失败",
            AppError::Internal(_) => "发生错误",
        }
    }

    /// 用户可理解的错误说明。
    pub fn user_message(&self) -> String {
        match self {
            AppError::NotFound(p) => {
                format!("找不到文件：{p}\n文件可能已被移动、重命名或删除。")
            }
            AppError::PermissionDenied(p) => {
                format!("没有权限读取：{p}\n请检查文件访问权限后重试。")
            }
            AppError::TooLarge(msg) => msg.clone(),
            AppError::BinaryFile => "该文件看起来是二进制文件，暂不支持直接阅读。".to_string(),
            AppError::UnknownEncoding => {
                "无法识别该文件的字符编码，暂时无法正确显示内容。".to_string()
            }
            AppError::FileChanged(_) => {
                "文件在读取过程中被修改（或替换），当前读取会话已自动关闭。\n请重新打开文件继续阅读。"
                    .to_string()
            }
            AppError::SessionNotFound => "读取会话不存在或已关闭，请重新打开文件。".to_string(),
            AppError::Io(msg) => msg.clone(),
            AppError::Internal(msg) => msg.clone(),
        }
    }

    /// 底层错误详情（仅用于日志，不展示给用户）。
    pub fn log_detail(&self) -> String {
        match self {
            AppError::Io(e) => format!("io error: {e}"),
            AppError::NotFound(e) => format!("not found: {e}"),
            AppError::PermissionDenied(e) => format!("permission denied: {e}"),
            AppError::FileChanged(e) => format!("file changed: {e}"),
            AppError::SessionNotFound => "session not found".to_string(),
            other => other.to_string(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("title", self.title())?;
        state.serialize_field("message", &self.user_message())?;
        state.serialize_field("code", &self.code())?;
        state.end()
    }
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "FILE_NOT_FOUND",
            AppError::PermissionDenied(_) => "FILE_PERMISSION_DENIED",
            AppError::TooLarge(_) => "FILE_TOO_LARGE",
            AppError::BinaryFile => "FILE_IS_BINARY",
            AppError::UnknownEncoding => "ENCODING_UNKNOWN",
            AppError::FileChanged(_) => "FILE_CHANGED",
            AppError::SessionNotFound => "SESSION_NOT_FOUND",
            AppError::Io(_) => "FILE_READ_FAILED",
            AppError::Internal(_) => "INTERNAL",
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound(e.to_string()),
            std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(e.to_string()),
            _ => AppError::Io(format!("读取文件失败：{e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_is_readable() {
        let err = AppError::NotFound("C:/x.md".into());
        assert!(err.user_message().contains("找不到"));
        assert_eq!(err.code(), "FILE_NOT_FOUND");
    }

    #[test]
    fn serializes_to_json_with_expected_fields() {
        let err = AppError::BinaryFile;
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "FILE_IS_BINARY");
        assert_eq!(json["title"], "不支持的文本格式");
        assert!(json["message"].as_str().unwrap().contains("二进制"));
    }

    #[test]
    fn file_changed_error_serializes_with_code() {
        let err = AppError::FileChanged("C:/big.txt".into());
        assert_eq!(err.code(), "FILE_CHANGED");
        assert!(err.user_message().contains("已自动关闭"));
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "FILE_CHANGED");
        assert_eq!(json["title"], "文件已变更");
    }

    #[test]
    fn session_not_found_error_serializes_with_code() {
        let err = AppError::SessionNotFound;
        assert_eq!(err.code(), "SESSION_NOT_FOUND");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "SESSION_NOT_FOUND");
    }
}