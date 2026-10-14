use std::path::{Path, PathBuf};

use crate::{
    bail,
    config::{StorageConfig, paths},
    domain::vo::{FileId, THUMB_FORMATS, THUMB_PIPELINE, THUMB_SIZES, ThumbFormat, ThumbSize},
    error::{AppError, ErrorCode, Result},
};

pub fn strip_control_chars(input: &str) -> String {
    // 控制字符可注入 `Content-Disposition` 与日志行。
    input.chars().filter(|c| !c.is_control()).collect()
}

/// 派生图在磁盘上的名字：`v{pipeline}-{longest_side}.{ext}`。
pub fn thumb_name(size: ThumbSize, format: ThumbFormat) -> String {
    // 版本进名字：改编码参数时旧产物才会被重新生成。
    format!("v{}-{}.{}", THUMB_PIPELINE, size.get(), format.ext())
}

/// 当前管线允许存在的全部派生图名字；`reconcile` 用它决定删什么。
pub fn thumb_names() -> Vec<String> {
    THUMB_SIZES
        .iter()
        .flat_map(|size| {
            THUMB_FORMATS
                .iter()
                .map(|format| thumb_name(*size, *format))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct MediaEntry {
    pub name: String,
    pub path: PathBuf,
    /// 该行 `origin` 的长度；目录里没有 `origin` 即 `None`。
    pub origin_len: Option<u64>,
    /// `origin` 的 mtime；目录里没有 `origin` 时退回目录自身的 mtime。
    pub modified: std::time::SystemTime,
}

/// 一个文件一格目录：`{media}/{uuid}/origin` 是原文件，`{media}/{uuid}/thumbs/` 放派生图。
/// 派生图不进库，因此随该目录生死：删行删目录，回收也以目录为单位。
#[derive(Debug, Clone)]
pub struct MediaStore {
    dir: PathBuf,
    max_file_size: u64,
}

impl MediaStore {
    pub fn new(storage: &StorageConfig) -> Self {
        Self {
            dir: paths::media_dir(storage),
            max_file_size: storage.max_file_size,
        }
    }

    pub async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| AppError::internal(format!("cannot create media dir: {e}")))?;
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn dir_of(&self, id: FileId) -> PathBuf {
        self.dir.join(id.to_string())
    }

    /// `{media}/{uuid}/origin`。
    pub fn origin_path(&self, id: FileId) -> PathBuf {
        self.dir_of(id).join("origin")
    }

    fn thumb_path(&self, id: FileId, name: &str) -> PathBuf {
        self.dir_of(id).join("thumbs").join(name)
    }

    pub async fn write(&self, id: FileId, data: &[u8]) -> Result<()> {
        if data.len() as u64 > self.max_file_size {
            bail!(AppError::business_msg(
                ErrorCode::FileTooLarge,
                format!("File exceeds the {} byte limit", self.max_file_size)
            ));
        }

        tokio::fs::create_dir_all(self.dir_of(id)).await?;
        tokio::fs::write(self.origin_path(id), data).await?;
        Ok(())
    }

    pub async fn open(&self, id: FileId) -> Result<Option<tokio::fs::File>> {
        match tokio::fs::File::open(self.origin_path(id)).await {
            Ok(file) => Ok(Some(file)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 先写临时文件再 `rename`：并发写同一档位只会互相覆盖，不会留下半截文件；
    /// 崩溃留下的临时文件不是期望集合里的名字，由 `reconcile` 清掉。
    pub async fn write_thumb(&self, id: FileId, name: &str, data: &[u8]) -> Result<()> {
        if data.len() as u64 > self.max_file_size {
            bail!(AppError::business_msg(
                ErrorCode::FileTooLarge,
                format!("File exceeds the {} byte limit", self.max_file_size)
            ));
        }

        let thumbs = self.dir_of(id).join("thumbs");
        tokio::fs::create_dir_all(&thumbs).await?;

        let tmp = thumbs.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&tmp, data).await?;
        tokio::fs::rename(&tmp, thumbs.join(name)).await?;
        Ok(())
    }

    pub async fn open_thumb(&self, id: FileId, name: &str) -> Result<Option<tokio::fs::File>> {
        match tokio::fs::File::open(self.thumb_path(id, name)).await {
            Ok(file) => Ok(Some(file)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// `thumbs/` 下的文件名（含遗留的临时文件）；目录不存在即空。
    pub async fn list_derivatives(&self, id: FileId) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let mut dir = match tokio::fs::read_dir(self.dir_of(id).join("thumbs")).await {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(e.into()),
        };

        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();

            if !entry.metadata().await?.is_file() {
                tracing::warn!(name, "skipping non-file entry in a thumbs directory");
                continue;
            }

            names.push(name);
        }

        Ok(names)
    }

    pub async fn delete_derivative(&self, id: FileId, name: &str) -> Result<()> {
        match tokio::fs::remove_file(self.thumb_path(id, name)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// 一条调用带走原文件与全部派生图。
    pub async fn delete(&self, id: FileId) -> Result<()> {
        match tokio::fs::remove_dir_all(self.dir_of(id)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// 每个直接子目录一条：`name` 是目录名（uuid），`origin_len` / `modified` 描述里面的 `origin`。
    /// 不做名字校验：不是 uuid 的目录照样返回，由 `reconcile` 当孤儿处理。
    pub async fn list(&self) -> Result<Vec<MediaEntry>> {
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.dir).await?;

        while let Some(entry) = dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().into_owned();

            if !metadata.is_dir() {
                tracing::warn!(name, "skipping non-directory entry in the media directory");
                continue;
            }

            let origin = entry.path().join("origin");
            let (origin_len, modified) = match tokio::fs::metadata(&origin).await {
                Ok(origin_metadata) => (Some(origin_metadata.len()), origin_metadata.modified()?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, metadata.modified()?),
                Err(e) => return Err(e.into()),
            };

            entries.push(MediaEntry {
                name,
                path: entry.path(),
                origin_len,
                modified,
            });
        }

        Ok(entries)
    }

    /// 只删除 `media` 目录的直接子目录：相对该目录必须恰是一个文件名，否则内部错误。
    pub async fn delete_dir(&self, path: &Path) -> Result<()> {
        let is_direct_child = path.strip_prefix(&self.dir).is_ok_and(|rel| {
            let mut components = rel.components();
            matches!(components.next(), Some(std::path::Component::Normal(_)))
                && components.next().is_none()
        });

        if !is_direct_child {
            bail!(AppError::internal(format!(
                "refusing to delete outside the media directory: {}",
                path.display()
            )));
        }

        match tokio::fs::remove_dir_all(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    fn temp_store(max_file_size: u64) -> (MediaStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("media-test-{}", uuid::Uuid::new_v4()));
        let store = MediaStore {
            dir: dir.clone(),
            max_file_size,
        };
        (store, dir)
    }

    #[test]
    fn strips_control_chars() {
        assert_eq!(strip_control_chars("a\r\nb.txt"), "ab.txt");
        assert_eq!(strip_control_chars("a\tb.txt"), "ab.txt");
        assert_eq!(strip_control_chars("正常.txt"), "正常.txt");
        assert_eq!(strip_control_chars(""), "");
    }

    #[test]
    fn thumb_names_covers_every_size_and_format() {
        assert_eq!(
            thumb_names(),
            [
                "v1-64.avif",
                "v1-64.jpg",
                "v1-128.avif",
                "v1-128.jpg",
                "v1-256.avif",
                "v1-256.jpg",
                "v1-512.avif",
                "v1-512.jpg",
                "v1-2048.avif",
                "v1-2048.jpg",
            ]
        );
    }

    #[tokio::test]
    async fn write_open_delete_roundtrip_and_size_limit() {
        let (store, dir) = temp_store(4);
        store.ensure_dir().await.unwrap();

        let id = FileId::new();
        store.write(id, b"hey").await.unwrap();
        assert_eq!(store.list().await.unwrap().len(), 1);

        let mut blob = store.open(id).await.unwrap().expect("blob exists");
        let mut read = Vec::new();
        blob.read_to_end(&mut read).await.unwrap();
        assert_eq!(read, b"hey");

        assert!(store.write(id, b"12345").await.is_err(), "over the limit");

        store.delete(id).await.unwrap();
        assert!(store.open(id).await.unwrap().is_none());
        store.delete(id).await.unwrap();
        assert!(store.list().await.unwrap().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn write_thumb_is_atomic_and_listed() {
        let (store, dir) = temp_store(1024);
        store.ensure_dir().await.unwrap();
        let id = FileId::new();
        store.write(id, b"origin").await.unwrap();

        store.write_thumb(id, "v1-64.avif", b"a").await.unwrap();
        let mut thumb = store
            .open_thumb(id, "v1-64.avif")
            .await
            .unwrap()
            .expect("thumb");
        let mut read = Vec::new();
        thumb.read_to_end(&mut read).await.unwrap();
        assert_eq!(read, b"a");
        assert_eq!(store.list_derivatives(id).await.unwrap(), ["v1-64.avif"]);

        // 崩溃留下的临时文件照样被列出（名字不在期望集合里，由 reconcile 负责清）。
        store
            .write_thumb(id, ".v1-64.avif.junk.tmp", b"b")
            .await
            .unwrap();
        assert!(
            store
                .list_derivatives(id)
                .await
                .unwrap()
                .contains(&".v1-64.avif.junk.tmp".to_string())
        );

        store.delete_derivative(id, "v1-64.avif").await.unwrap();
        assert!(store.open_thumb(id, "v1-64.avif").await.unwrap().is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn the_origin_length_and_mtime_describe_an_entry() {
        let (store, dir) = temp_store(1024);
        store.ensure_dir().await.unwrap();

        let id = FileId::new();
        store.write(id, b"hey").await.unwrap();
        let entry = store.list().await.unwrap().pop().expect("one entry");
        assert_eq!(entry.name, id.to_string());
        assert_eq!(entry.origin_len, Some(3));

        // 有行无 blob（here: 目录在、origin 没了）只报长度缺失，宽限期退回目录 mtime。
        tokio::fs::remove_file(store.origin_path(id)).await.unwrap();
        let entry = store.list().await.unwrap().pop().expect("still one entry");
        assert_eq!(entry.origin_len, None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn delete_dir_refuses_paths_outside_the_media_directory() {
        let (store, dir) = temp_store(4);
        store.ensure_dir().await.unwrap();

        let outside = dir.join("..").join("outside");
        tokio::fs::create_dir_all(&outside).await.unwrap();
        assert!(store.delete_dir(&outside).await.is_err());
        assert!(outside.exists());

        let id = FileId::new();
        store.write(id, b"hey").await.unwrap();
        store
            .delete_dir(&store.dir().join(id.to_string()))
            .await
            .unwrap();
        assert!(store.open(id).await.unwrap().is_none());

        std::fs::remove_dir_all(outside).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn delete_dir_refuses_a_plain_file() {
        let (store, dir) = temp_store(4);
        store.ensure_dir().await.unwrap();

        let stray = dir.join("stray");
        tokio::fs::write(&stray, b"x").await.unwrap();
        assert!(
            store.delete_dir(&stray).await.is_err(),
            "a direct child that is not a directory is not an entry"
        );
        assert!(stray.exists());

        std::fs::remove_file(stray).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    // 媒体目录自身带 `..` 时，条目路径的前缀里也有 `ParentDir`，只有相对该目录的判断才拦得住越界。
    #[tokio::test]
    async fn delete_dir_accepts_a_media_dir_that_contains_dot_dot() {
        let base = std::env::temp_dir().join(format!("media-test-{}", uuid::Uuid::new_v4()));
        let dir = base.join("sub").join("..").join("media");
        let store = MediaStore {
            dir: dir.clone(),
            max_file_size: 4,
        };
        store.ensure_dir().await.unwrap();

        let id = FileId::new();
        store.write(id, b"hey").await.unwrap();

        let entry = store.list().await.unwrap().pop().expect("one blob");
        store.delete_dir(&entry.path).await.unwrap();
        assert!(store.open(id).await.unwrap().is_none());

        std::fs::remove_dir_all(&base).ok();
    }
}
