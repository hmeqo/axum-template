use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app::extractor::{UploadedFile, multipart::MultipartReader},
    domain::{
        service::{StoredFile, thumbnail::is_thumbnailable},
        vo::{FileId, FileStateCode, Page, THUMB_DEFAULT},
    },
    error::AppError,
};

#[derive(Debug, ToSchema)]
pub struct UploadForm {
    /// The part's `filename` becomes `original_name` and its `Content-Type` becomes `mime_type`,
    /// defaulting to `application/octet-stream` when the part declares none.
    #[schema(value_type = String, format = Binary)]
    pub file: UploadedFile,
}

#[async_trait::async_trait]
impl crate::app::extractor::FromMultipart for UploadForm {
    async fn from_multipart(reader: &mut MultipartReader) -> Result<Self, AppError> {
        let field = reader
            .take_file("file")
            .ok_or_else(|| AppError::malformed_request("Missing 'file' field"))?;

        Ok(UploadForm {
            file: UploadedFile {
                name: field.file_name.unwrap_or_default(),
                mime_type: field
                    .content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                data: field.data.unwrap_or_default(),
            },
        })
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileResp {
    /// Opaque file handle; the download path is `{media-url-prefix}/{id}`.
    pub id: FileId,
    /// Link to the original file.
    pub url: String,
    /// Link to a `?size=256` derivative; `null` when the file is not a raster image.
    pub thumbnail_url: Option<String>,
    pub original_name: String,
    pub mime_type: String,
    pub size: u64,
    /// Display width of the original (EXIF orientation applied); `null` for non-images.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// sha256 (lowercase hex) of the content; it is also the `ETag` once the file is attached.
    pub hash: String,
    pub state: FileStateCode,
    #[schema(value_type = String, format = "date-time")]
    pub attached_at: Option<jiff::Timestamp>,
    #[schema(value_type = String, format = "date-time")]
    pub created_at: jiff::Timestamp,
}

impl FileResp {
    pub fn from_stored(stored: &StoredFile, media_url_prefix: &str) -> Self {
        let file = &stored.file;
        let url = format!("{media_url_prefix}/{}", file.id);

        Self {
            thumbnail_url: is_thumbnailable(&file.mime_type)
                .then(|| format!("{url}?size={THUMB_DEFAULT}")),
            id: file.id,
            width: file.width,
            height: file.height,
            url,
            original_name: file.original_name.clone(),
            mime_type: file.mime_type.clone(),
            size: file.file_size as u64,
            hash: file.hash.clone(),
            state: stored.state.code(),
            attached_at: stored.state.attached_at(),
            created_at: file.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FileListResp {
    pub files: Vec<FileResp>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl From<Page<FileResp>> for FileListResp {
    fn from(page: Page<FileResp>) -> Self {
        let (total, page_number, per_page) = (page.total(), page.page(), page.per_page());
        Self {
            files: page.items,
            total,
            page: page_number,
            per_page,
        }
    }
}
