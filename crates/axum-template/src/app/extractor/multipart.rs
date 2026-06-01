use axum::extract::{FromRequest, Request, multipart::Multipart};
use derive_more::{Deref, DerefMut};
use utoipa::ToSchema;

use crate::error::{AppError, ErrorKind};

// ── Internal multipart field reader ──

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
            let file_name = field.file_name().map(|s| s.to_string());
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

    pub fn file(&self, name: &str) -> Option<&MultipartField> {
        self.fields
            .iter()
            .find(|f| f.name == name && f.data.is_some())
    }
}

// ── Uploaded file ──

#[derive(Debug, ToSchema)]
pub struct UploadedFile {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

// ── FormData extractor ──

/// Extractor that parses multipart/form-data into a type implementing `FromMultipart`.
///
/// # Example
///
/// ```ignore
/// async fn upload(FormData(form): FormData<MyUploadForm>) -> Result<...> {
///     // use form.title, form.file, etc.
/// }
/// ```
#[derive(Deref, DerefMut)]
pub struct FormData<T>(pub T);

/// Trait for types that can be parsed from multipart form data via `FormData<T>`.
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
            .map_err(|e| ErrorKind::ValidationFailed.msg(format!("Invalid multipart: {}", e)))?;
        let mut reader = MultipartReader::read(&mut multipart).await.map_err(|e| {
            ErrorKind::ValidationFailed.msg(format!("Failed to read multipart: {}", e))
        })?;
        let value = T::from_multipart(&mut reader).await?;
        Ok(FormData(value))
    }
}
