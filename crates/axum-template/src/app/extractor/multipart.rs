use axum::{
    extract::{
        FromRequest, Request,
        multipart::{Multipart, MultipartError},
    },
    http::StatusCode,
};

use crate::{
    error::{AppError, ErrorCode},
    infra::storage::strip_control_chars,
};

pub struct MultipartReader {
    fields: Vec<MultipartField>,
}

#[derive(Debug)]
pub struct MultipartField {
    pub name: String,
    pub text: Option<String>,
    pub file_name: Option<String>,
    pub content_type: Option<String>,
    pub data: Option<Vec<u8>>,
}

impl MultipartReader {
    pub async fn read(
        multipart: &mut Multipart,
    ) -> Result<Self, axum::extract::multipart::MultipartError> {
        let mut fields = Vec::new();
        while let Some(field) = multipart.next_field().await? {
            let name = field.name().unwrap_or("").to_string();
            let file_name = field.file_name().map(strip_control_chars);
            let content_type = field.content_type().map(|s| s.to_string());

            if file_name.is_some() {
                let data = field.bytes().await?.to_vec();
                fields.push(MultipartField {
                    name,
                    text: None,
                    file_name,
                    content_type,
                    data: Some(data),
                });
            } else {
                let text = field.text().await?;
                fields.push(MultipartField {
                    name,
                    text: Some(text),
                    file_name: None,
                    content_type,
                    data: None,
                });
            }
        }
        Ok(Self { fields })
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name == name && f.text.is_some())
            .and_then(|f| f.text.as_deref())
    }

    pub fn take_file(&mut self, name: &str) -> Option<MultipartField> {
        let index = self
            .fields
            .iter()
            .position(|f| f.name == name && f.data.is_some())?;
        Some(self.fields.remove(index))
    }
}

#[derive(Debug)]
pub struct UploadedFile {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

pub struct FormData<T>(pub T);

#[async_trait::async_trait]
pub trait FromMultipart: Sized {
    async fn from_multipart(reader: &mut MultipartReader) -> Result<Self, AppError>;
}

impl<T, S> FromRequest<S> for FormData<T>
where
    T: FromMultipart,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut multipart = Multipart::from_request(req, state)
            .await
            .map_err(|e| AppError::malformed_request(format!("Invalid multipart: {e}")))?;
        let mut reader = MultipartReader::read(&mut multipart)
            .await
            .map_err(|e| multipart_error("Failed to read multipart", e))?;
        let value = T::from_multipart(&mut reader).await?;
        Ok(FormData(value))
    }
}

/// 超体积上限（413）与 `MediaStore::write` 的超限给同一个码；其余按请求格式错处理。
fn multipart_error(what: &str, e: MultipartError) -> AppError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::business(ErrorCode::FileTooLarge)
    } else {
        AppError::malformed_request(format!("{what}: {e}"))
    }
}
