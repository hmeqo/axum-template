use super::{AppError, ErrorCode, FieldErrors};

impl From<toasty::Error> for AppError {
    fn from(e: toasty::Error) -> Self {
        if e.is_connection_lost() || e.is_connection_pool() {
            Self::Unavailable {
                source: Box::new(e),
            }
        } else {
            Self::Database(e)
        }
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(e: validator::ValidationErrors) -> Self {
        let errors: FieldErrors = e
            .field_errors()
            .iter()
            .map(|(field, kinds)| {
                let messages = kinds
                    .iter()
                    .map(|kind| match kind.message.as_ref() {
                        Some(m) => m.to_string(),
                        None => kind.code.to_string(),
                    })
                    .collect();
                ((*field).to_string(), messages)
            })
            .collect();

        Self::Business {
            code: ErrorCode::InputInvalidFields,
            message: ErrorCode::InputInvalidFields.message().into(),
            errors: Some(errors),
        }
    }
}
