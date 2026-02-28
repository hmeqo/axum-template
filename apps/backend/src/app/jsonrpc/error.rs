use jsonrpsee::types::ErrorObject;
use serde_json::json;

use crate::error::AppError;

impl From<AppError> for ErrorObject<'static> {
    fn from(err: AppError) -> Self {
        err.trace_source();
        ErrorObject::owned(
            -32000,
            err.code(),
            Some(json!({
                "message": err.message()
            })),
        )
    }
}
