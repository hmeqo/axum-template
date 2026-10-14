use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use sha2::Digest;
use toasty::Db;
use tokio::{io::AsyncReadExt, sync::Semaphore};

use crate::{
    bail,
    domain::{
        db::ToastyResultExt,
        model::File,
        service::thumbnail as thumbs,
        vo::{FileId, FileState, FileStateCode, Page, Pagination, ThumbFormat, ThumbSize, UserId},
    },
    error::{AppError, Result},
    infra::storage::{MediaEntry, MediaStore, thumb_name, thumb_names},
};

/// 行 + 类型化生命周期；`state` 是 (file.state, file.attached_at) 的唯一解析结果。
#[derive(Debug, Clone)]
pub struct StoredFile {
    pub file: File,
    pub state: FileState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub orphans_deleted: u64,
    /// 活目录里不属于当前 `(版本, 档位, 格式)` 集合的派生文件（含失败留下的临时文件）。
    pub derivatives_deleted: u64,
    pub missing_blobs: u64,
    pub size_mismatches: u64,
}

/// 一份就绪下发的派生图：行 + 打开的文件句柄 + 派生文件长度（响应头不能再用行里的 `file_size`）。
#[derive(Debug)]
pub struct Thumbnail {
    pub file: StoredFile,
    pub size: ThumbSize,
    pub format: ThumbFormat,
    pub body: tokio::fs::File,
    pub len: u64,
}

#[derive(Debug, Clone)]
pub struct FileService {
    db: Db,
    media: MediaStore,
    // 渲染是 CPU 密集的阻塞任务：进程内并发在这里封顶（rav1e 自身线程池已按 1 线程关掉）。
    thumbs: Arc<Semaphore>,
}

impl FileService {
    pub fn new(db: Db, media: MediaStore) -> Self {
        let permits = std::thread::available_parallelism().map_or(2, |n| n.get());

        Self {
            db,
            media,
            thumbs: Arc::new(Semaphore::new(permits)),
        }
    }

    /// 先盘后行：写盘失败无脏行；建行失败留孤儿盘文件，由 [`Self::reconcile`] 回收。
    pub async fn upload(
        &self,
        user_id: UserId,
        original_name: String,
        mime_type: String,
        data: &[u8],
    ) -> Result<StoredFile> {
        let id = FileId::new();
        let hash = sha256_hex(data);
        // 只读图片头：给消费端留出排布所需的宽高，失败就是非图片（列保持 NULL）。
        let (width, height) = match thumbs::probe_dimensions(data) {
            Some((width, height)) => (Some(width), Some(height)),
            None => (None, None),
        };

        self.media.write(id, data).await?;

        let mut db = self.db.clone();
        let state = FileState::Draft;
        let file = toasty::create!(File {
            id,
            user_id,
            original_name,
            mime_type,
            file_size: data.len() as i64,
            width,
            height,
            hash,
            state: state.code(),
            attached_at: state.attached_at(),
        })
        .exec(&mut db)
        .await?;

        self.wrap(file)
    }

    /// 幂等：已采用时原样返回；非所有者调用即拒绝。
    pub async fn attach(&self, user_id: UserId, id: FileId) -> Result<StoredFile> {
        let mut db = self.db.clone();
        let mut file = self
            .find_row(&mut db, id)
            .await?
            .ok_or_else(|| AppError::not_found("File not found"))?;

        if file.user_id != user_id {
            bail!(AppError::forbidden("Cannot attach another user's file"));
        }

        let state = FileState::from_columns(file.state, file.attached_at)?;
        if state.is_attached() {
            return Ok(StoredFile { file, state });
        }

        let attached = FileState::Attached {
            at: jiff::Timestamp::now(),
        };
        toasty::update!(file {
            state: attached.code(),
            attached_at: attached.attached_at()
        })
        .exec(&mut db)
        .await?;

        // 重新取行:内存里的 `attached_at` 是纳秒,库里是微秒,直接返回会让两次 attach 的响应不一致。
        let updated = self
            .find_row(&mut db, id)
            .await?
            .ok_or_else(|| AppError::not_found("File not found"))?;
        self.wrap(updated)
    }

    /// 该用户自己的文件，最新的在前。
    pub async fn list(&self, user_id: UserId, pagination: Pagination) -> Result<Vec<StoredFile>> {
        let mut db = self.db.clone();
        let files = File::all()
            .filter(File::fields().user_id().eq(user_id))
            .order_by(File::fields().created_at().desc())
            .order_by(File::fields().id().desc())
            .limit(pagination.limit())
            .offset(pagination.offset())
            .exec(&mut db)
            .await?;

        files.into_iter().map(|file| self.wrap(file)).collect()
    }

    pub async fn count(&self, user_id: UserId) -> Result<u64> {
        let mut db = self.db.clone();
        Ok(File::all()
            .filter(File::fields().user_id().eq(user_id))
            .count()
            .exec(&mut db)
            .await?)
    }

    /// 两个查询都限定在这位调用者名下。
    pub async fn page(&self, user_id: UserId, pagination: Pagination) -> Result<Page<StoredFile>> {
        let items = self.list(user_id, pagination).await?;
        let total = self.count(user_id).await?;
        Ok(Page::new(items, total, pagination))
    }

    pub async fn find_by_id(&self, id: FileId) -> Result<Option<StoredFile>> {
        let mut db = self.db.clone();
        match self.find_row(&mut db, id).await? {
            Some(file) => self.wrap(file).map(Some),
            None => Ok(None),
        }
    }

    /// 命中磁盘即返回；未命中现场生成后返回。派生图不进库，随 `{uuid}` 目录生死。
    pub async fn thumbnail(
        &self,
        id: FileId,
        size: ThumbSize,
        format: ThumbFormat,
    ) -> Result<Thumbnail> {
        let stored = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::not_found("File not found"))?;

        if !thumbs::is_thumbnailable(&stored.file.mime_type) {
            bail!(AppError::malformed_request("File is not an image"));
        }

        let name = thumb_name(size, format);
        if let Some(body) = self.media.open_thumb(id, &name).await? {
            let len = body.metadata().await?.len();
            return Ok(Thumbnail {
                file: stored,
                size,
                format,
                body,
                len,
            });
        }

        let mut origin = self
            .media
            .open(id)
            .await?
            .ok_or_else(|| AppError::internal(format!("file blob is missing: {id}")))?;
        // 上限即 `storage.max-file-size`，因此这里不可能无界读。
        let mut source = Vec::new();
        origin.read_to_end(&mut source).await?;

        let permit = self
            .thumbs
            .acquire()
            .await
            .map_err(|_| AppError::internal("thumbnail semaphore is closed"))?;
        let rendered = tokio::task::spawn_blocking(move || thumbs::render(&source, size, format))
            .await
            .map_err(|e| AppError::internal(format!("thumbnail task failed: {e}")))??;
        drop(permit);

        self.media.write_thumb(id, &name, &rendered).await?;

        let body = self
            .media
            .open_thumb(id, &name)
            .await?
            .ok_or_else(|| AppError::internal(format!("thumbnail vanished: {id} {name}")))?;
        let len = body.metadata().await?.len();

        Ok(Thumbnail {
            file: stored,
            size,
            format,
            body,
            len,
        })
    }

    /// 单行失败只记日志并跳过，返回成功回收的条数。
    pub async fn sweep_unattached(&self, ttl: Duration) -> Result<u64> {
        let ttl_secs = i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX);
        let cutoff = jiff::Timestamp::now() + jiff::Span::new().seconds(-ttl_secs);
        let mut db = self.db.clone();

        let drafts = File::all()
            .filter(File::fields().state().eq(FileStateCode::Draft))
            .filter(File::fields().created_at().lt(cutoff))
            .exec(&mut db)
            .await?;

        let mut removed = 0;
        for draft in drafts {
            let id = draft.id;
            let outcome = async {
                File::delete_by_id(&mut db, id).await?;
                self.media.delete(id).await
            }
            .await;

            match outcome {
                Ok(()) => removed += 1,
                Err(e) => tracing::warn!(%id, error = %e, "unattached draft cleanup failed"),
            }
        }

        Ok(removed)
    }

    /// 只比大小，不重算内容。
    pub async fn reconcile(&self, grace: Duration) -> Result<ReconcileReport> {
        let mut db = self.db.clone();
        let rows = File::all().exec(&mut db).await?;
        let rows_by_id: HashMap<String, &File> =
            rows.iter().map(|row| (row.id.to_string(), row)).collect();

        let mut report = ReconcileReport::default();
        let mut rows_with_blob = HashSet::new();

        for entry in self.media.list().await? {
            match rows_by_id.get(&entry.name) {
                Some(row) => {
                    if let Some(len) = entry.origin_len {
                        rows_with_blob.insert(row.id);
                        if len as i64 != row.file_size {
                            report.size_mismatches += 1;
                            tracing::warn!(
                                id = %row.id,
                                row_size = row.file_size,
                                blob_size = len,
                                "file size does not match its row"
                            );
                        }
                    }

                    report.derivatives_deleted += self.prune_derivatives(row.id).await?;
                }
                None => {
                    report.orphans_deleted += u64::from(self.delete_orphan(&entry, grace).await?)
                }
            }
        }

        for row in &rows {
            if !rows_with_blob.contains(&row.id) {
                report.missing_blobs += 1;
                tracing::warn!(id = %row.id, "file row has no blob");
            }
        }

        Ok(report)
    }

    /// 删掉活目录里不属于当前管线期望集合的派生文件（含临时文件）；单条失败只警告不中断。
    async fn prune_derivatives(&self, id: FileId) -> Result<u64> {
        let expected = thumb_names();
        let mut removed = 0;

        for name in self.media.list_derivatives(id).await? {
            if expected.contains(&name) {
                continue;
            }

            match self.media.delete_derivative(id, &name).await {
                Ok(()) => {
                    removed += 1;
                    tracing::info!(%id, name, "deleted a stale derivative");
                }
                Err(e) => tracing::warn!(%id, name, error = %e, "derivative cleanup failed"),
            }
        }

        Ok(removed)
    }

    async fn delete_orphan(&self, entry: &MediaEntry, grace: Duration) -> Result<bool> {
        if entry.modified.elapsed().unwrap_or_default() <= grace {
            return Ok(false);
        }

        self.media.delete_dir(&entry.path).await?;
        tracing::info!(name = %entry.name, "deleted an orphan media directory");
        Ok(true)
    }

    fn wrap(&self, file: File) -> Result<StoredFile> {
        let state = FileState::from_columns(file.state, file.attached_at)?;
        Ok(StoredFile { file, state })
    }

    async fn find_row(&self, db: &mut Db, id: FileId) -> Result<Option<File>> {
        File::filter_by_id(id).get(db).await.optional()
    }
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(data))
}
