//! Shared resource export, including provenance, integrity and structured failures.
//! Tolerant link helpers preserve the original reference on failure. Fallible helpers
//! return the same recorded error; strict batches are checked after rendering.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default)]
pub struct PayloadSinks<'a> {
    pub attachments: Option<&'a PayloadSink>,
    pub media: Option<&'a PayloadSink>,
}

impl PayloadSinks<'_> {
    /// Scope model locations (e.g. /part[1]/block[2]) through nested renderers.
    pub fn at_source(&self, component: impl AsRef<str>) -> Vec<SourceScope<'_>> {
        let mut scopes = Vec::new();
        if let Some(sink) = self.attachments {
            scopes.push(sink.at_source(component.as_ref()));
        }
        if let Some(sink) = self.media {
            if !self
                .attachments
                .is_some_and(|other| std::ptr::eq(other, sink))
            {
                scopes.push(sink.at_source(component.as_ref()));
            }
        }
        scopes
    }

    /// Add a summary to the existing result envelope; retain all per-resource errors.
    /// Returns true when strict mode requires a nonzero exit status.
    pub fn annotate_report(&self, value: &mut serde_json::Value) -> bool {
        let mut entries = Vec::new();
        let mut strict = false;
        let mut failed = false;
        let distinct_media = self.media.filter(|media| {
            !self
                .attachments
                .is_some_and(|attachment| std::ptr::eq(*media, attachment))
        });
        for sink in [self.attachments, distinct_media].into_iter().flatten() {
            entries.extend(sink.entries());
            strict |= sink.strict;
            failed |= sink.check().is_err();
        }
        if self.attachments.is_some() || self.media.is_some() {
            let errors = entries.iter().filter(|entry| entry.error.is_some()).count();
            value["export_summary"] = serde_json::json!({
                "resources": entries.len(), "exported": entries.iter().filter(|e| e.kind == "file").count(),
                "failed": errors, "strict": strict,
            });
            if failed {
                value["ok"] = serde_json::json!(false);
                value["error"] = serde_json::json!({"code":"resource_export_failed", "message":format!("资源导出失败（{errors} 项），详见导出清单")});
            }
        }
        failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportErrorCode {
    PayloadMissing,
    SourceReadFailed,
    CreateDirFailed,
    CopyFailed,
    WriteFailed,
    InvalidName,
    InvalidPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportError {
    pub code: ExportErrorCode,
    pub operation: String,
    pub message: String,
    pub io_kind: Option<String>,
    pub os_code: Option<i32>,
    pub target: Option<String>,
}

impl std::fmt::Display for ExportErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::PayloadMissing => "payload_missing",
            Self::SourceReadFailed => "source_read_failed",
            Self::CreateDirFailed => "create_dir_failed",
            Self::CopyFailed => "copy_failed",
            Self::WriteFailed => "write_failed",
            Self::InvalidName => "invalid_name",
            Self::InvalidPayload => "invalid_payload",
        };
        f.write_str(code)
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for ExportError {}

impl ExportError {
    fn io(code: ExportErrorCode, operation: &str, error: io::Error, target: Option<&Path>) -> Self {
        Self {
            code,
            operation: operation.into(),
            message: error.to_string(),
            io_kind: Some(format!("{:?}", error.kind())),
            os_code: error.raw_os_error(),
            target: target.map(|path| path.display().to_string()),
        }
    }
}

/// One physical resource; repeated references add source locations instead of copies.
#[derive(Debug, Clone, Serialize)]
pub struct ExportedAttachment {
    pub resource_id: String,
    /// MIME type, inferred from the filename unless supplied by the format reader.
    pub resource_type: String,
    pub name: String,
    pub src: String,
    pub source_document: Option<String>,
    pub source_locations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// file / url / missing (only absent payloads) / error (other failures).
    pub kind: &'static str,
    /// Actual exported bytes, or known source size on failure. Unknown for URLs.
    pub size: Option<u64>,
    /// SHA-256 of the bytes actually written; unknown when copying did not finish.
    pub sha256: Option<String>,
    /// URI-escaped filename relative to the requested export directory.
    pub relative_link: Option<String>,
    pub error: Option<ExportError>,
}

impl ExportedAttachment {
    pub fn status_message(&self, label: &str) -> String {
        match &self.error {
            Some(error) => format!(
                "{label}导出失败: {} [{}] {}",
                self.src, error.code, error.message
            ),
            None if self.kind == "url" => format!("{label}是 URL，原样保留: {}", self.src),
            _ => format!(
                "{label}已导出: {} → {}",
                self.name,
                self.path.as_deref().unwrap_or("")
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportBatchError {
    pub failures: Vec<ExportedAttachment>,
}
impl std::fmt::Display for ExportBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} resources failed to export", self.failures.len())
    }
}
impl std::error::Error for ExportBatchError {}

pub struct PayloadSink {
    root: Option<PathBuf>,
    dir: PathBuf,
    strict: bool,
    source_document: Option<String>,
    location: RefCell<Vec<String>>,
    cache: RefCell<HashMap<String, usize>>,
    entries: RefCell<Vec<ExportedAttachment>>,
}

/// Restores the caller's source location when a nested renderer returns.
pub struct SourceScope<'a> {
    sink: &'a PayloadSink,
    previous_len: usize,
}
impl Drop for SourceScope<'_> {
    fn drop(&mut self) {
        self.sink.location.borrow_mut().truncate(self.previous_len);
    }
}

impl PayloadSink {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            root: None,
            dir: dir.into(),
            strict: false,
            source_document: None,
            location: RefCell::new(Vec::new()),
            cache: RefCell::new(HashMap::new()),
            entries: RefCell::new(Vec::new()),
        }
    }
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
    pub fn with_source_document(mut self, source: impl Into<String>) -> Self {
        self.source_document = Some(source.into());
        self
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn entries(&self) -> Vec<ExportedAttachment> {
        self.entries.borrow().clone()
    }
    pub fn check(&self) -> Result<(), ExportBatchError> {
        let failures: Vec<_> = self
            .entries
            .borrow()
            .iter()
            .filter(|e| e.error.is_some())
            .cloned()
            .collect();
        if self.strict && !failures.is_empty() {
            Err(ExportBatchError { failures })
        } else {
            Ok(())
        }
    }
    pub fn at_source(&self, component: impl AsRef<str>) -> SourceScope<'_> {
        let mut location = self.location.borrow_mut();
        let previous_len = location.len();
        location.push(component.as_ref().trim_matches('/').to_string());
        SourceScope {
            sink: self,
            previous_len,
        }
    }
    fn source_location(&self, fallback: &str) -> String {
        let location = self.location.borrow();
        if location.is_empty() {
            fallback.into()
        } else {
            format!("/{}", location.join("/"))
        }
    }
    fn payload_path(&self, src: &str) -> PathBuf {
        let path = Path::new(src);
        if path.is_absolute() {
            path.into()
        } else {
            self.root
                .as_ref()
                .map(|root| root.join(path))
                .unwrap_or_else(|| path.into())
        }
    }
    fn file_key(&self, src: &str) -> String {
        fn identity(path: &Path) -> PathBuf {
            if let Ok(path) = path.canonicalize() {
                return path;
            }
            match (path.parent(), path.file_name()) {
                (Some(parent), Some(name)) => identity(if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                })
                .join(name),
                _ => path.to_path_buf(),
            }
        }
        format!("file:{}", identity(&self.payload_path(src)).display())
    }
    fn cached(&self, key: &str, src: &str) -> Option<Result<String, ExportError>> {
        let index = *self.cache.borrow().get(key)?;
        let location = self.source_location(src);
        let mut entries = self.entries.borrow_mut();
        let entry = &mut entries[index];
        if !entry.source_locations.contains(&location) {
            entry.source_locations.push(location);
        }
        Some(match &entry.error {
            Some(error) => Err(error.clone()),
            None => Ok(entry.path.clone().unwrap_or_else(|| entry.src.clone())),
        })
    }
    fn entry(
        &self,
        name: &str,
        src: &str,
        identity: &str,
        mime: Option<&str>,
    ) -> ExportedAttachment {
        // Package paths remain stable when a binary input is unpacked to a new temp directory.
        let identity = if let Some(root) = &self.root {
            let root = root.canonicalize().unwrap_or_else(|_| root.clone());
            identity
                .strip_prefix("file:")
                .and_then(|path| Path::new(path).strip_prefix(&root).ok())
                .map(|path| format!("file:{}", path.display()))
                .unwrap_or_else(|| identity.into())
        } else {
            identity.into()
        };
        let id = format!(
            "{}\0{identity}",
            self.source_document.as_deref().unwrap_or("")
        );
        ExportedAttachment {
            resource_id: format!("resource-{:x}", Sha256::digest(id.as_bytes())),
            resource_type: mime
                .filter(|mime| !mime.trim().is_empty())
                .unwrap_or_else(|| infer_type(name))
                .into(),
            name: name.into(),
            src: src.into(),
            source_document: self.source_document.clone(),
            source_locations: vec![self.source_location(src)],
            path: None,
            kind: "error",
            size: None,
            sha256: None,
            relative_link: None,
            error: None,
        }
    }
    fn record(&self, key: String, entry: ExportedAttachment) {
        let index = self.entries.borrow().len();
        self.entries.borrow_mut().push(entry);
        self.cache.borrow_mut().insert(key, index);
    }
    fn failed(
        &self,
        key: String,
        mut entry: ExportedAttachment,
        error: ExportError,
    ) -> ExportError {
        entry.kind = if error.code == ExportErrorCode::PayloadMissing {
            "missing"
        } else {
            "error"
        };
        entry.error = Some(error.clone());
        self.record(key, entry);
        error
    }
    /// Record a decoder/missing-payload error discovered by a format-specific reader.
    pub fn report_failure(
        &self,
        name: &str,
        src: &str,
        code: ExportErrorCode,
        message: impl Into<String>,
    ) -> String {
        self.report_failure_typed(name, src, code, message, None)
    }
    pub fn report_failure_typed(
        &self,
        name: &str,
        src: &str,
        code: ExportErrorCode,
        message: impl Into<String>,
        mime: Option<&str>,
    ) -> String {
        let key = format!("bytes:{src}");
        if self.cached(&key, src).is_none() {
            let entry = self.entry(name, src, &key, mime);
            self.failed(
                key,
                entry,
                ExportError {
                    code,
                    operation: "decode".into(),
                    message: message.into(),
                    io_kind: None,
                    os_code: None,
                    target: None,
                },
            );
        }
        src.into()
    }
    pub fn link_for_url(&self, name: &str, src: &str) -> String {
        let key = format!("url:{src}");
        if let Some(Ok(link)) = self.cached(&key, src) {
            return link;
        }
        let mut entry = self.entry(name, src, &key, None);
        entry.kind = "url";
        self.record(key, entry);
        src.into()
    }
    pub fn link_for(&self, name: &str, src: &str) -> String {
        self.try_link_for(name, src).unwrap_or_else(|_| src.into())
    }
    pub fn try_link_for(&self, name: &str, src: &str) -> Result<String, ExportError> {
        self.try_link_for_typed(name, src, None)
    }
    pub fn try_link_for_typed(
        &self,
        name: &str,
        src: &str,
        mime: Option<&str>,
    ) -> Result<String, ExportError> {
        if is_url(src) {
            return Ok(self.link_for_url(name, src));
        }
        let key = self.file_key(src);
        if let Some(hit) = self.cached(&key, src) {
            return hit;
        }
        let mut entry = self.entry(name, src, &key, mime);
        let payload = self.payload_path(src);
        let source = std::fs::File::open(&payload).and_then(|file| {
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "载荷不是普通文件",
                ));
            }
            entry.size = Some(meta.len());
            Ok(file)
        });
        let mut source = match source {
            Ok(source) => source,
            Err(error) => {
                let code = if error.kind() == io::ErrorKind::NotFound {
                    ExportErrorCode::PayloadMissing
                } else {
                    ExportErrorCode::SourceReadFailed
                };
                return Err(self.failed(
                    key,
                    entry,
                    ExportError::io(code, "open_source", error, Some(&payload)),
                ));
            }
        };
        self.export(key, entry, &mut source, ExportErrorCode::CopyFailed)
    }
    /// Base64-backed package parts use the same manifest and strict checks as files.
    pub fn link_for_base64(
        &self,
        name: &str,
        encoded: &str,
        src: &str,
        mime: Option<&str>,
    ) -> String {
        use base64::Engine;
        let encoded: String = encoded
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(bytes) => self
                .try_link_for_bytes_typed(name, &bytes, src, mime)
                .unwrap_or_else(|_| src.into()),
            Err(error) => self.report_failure_typed(
                name,
                src,
                ExportErrorCode::InvalidPayload,
                error.to_string(),
                mime,
            ),
        }
    }
    pub fn link_for_bytes(&self, name: &str, bytes: &[u8], src: &str) -> String {
        self.try_link_for_bytes(name, bytes, src)
            .unwrap_or_else(|_| src.into())
    }
    pub fn try_link_for_bytes(
        &self,
        name: &str,
        bytes: &[u8],
        src: &str,
    ) -> Result<String, ExportError> {
        self.try_link_for_bytes_typed(name, bytes, src, None)
    }
    pub fn try_link_for_bytes_typed(
        &self,
        name: &str,
        bytes: &[u8],
        src: &str,
        mime: Option<&str>,
    ) -> Result<String, ExportError> {
        let key = format!("bytes:{src}");
        if let Some(hit) = self.cached(&key, src) {
            return hit;
        }
        let mut entry = self.entry(name, src, &key, mime);
        entry.size = Some(bytes.len() as u64);
        self.export(key, entry, &mut &bytes[..], ExportErrorCode::WriteFailed)
    }
    fn export(
        &self,
        key: String,
        mut entry: ExportedAttachment,
        source: &mut dyn Read,
        code: ExportErrorCode,
    ) -> Result<String, ExportError> {
        if !valid_name(&entry.name) {
            return Err(self.failed(
                key,
                entry,
                ExportError {
                    code: ExportErrorCode::InvalidName,
                    operation: "validate_name".into(),
                    message: "导出文件名必须是单个文件名，不能包含目录、驱动器或路径跳转".into(),
                    io_kind: None,
                    os_code: None,
                    target: None,
                },
            ));
        }
        if let Err(error) = std::fs::create_dir_all(&self.dir) {
            return Err(self.failed(
                key,
                entry,
                ExportError::io(
                    ExportErrorCode::CreateDirFailed,
                    "create_dir",
                    error,
                    Some(&self.dir),
                ),
            ));
        }
        // create_new prevents overwrites even if another exporter races our name check.
        let mut counter = 1u64;
        let (target, mut output) = loop {
            let (stem, ext) = split_ext(&entry.name);
            let name = if counter == 1 {
                entry.name.clone()
            } else {
                format!("{stem}-{counter}{ext}")
            };
            let target = self.dir.join(name);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
            {
                Ok(output) => break (target, output),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    counter += 1;
                }
                Err(error) => {
                    return Err(self.failed(
                        key,
                        entry,
                        ExportError::io(code, "create_target", error, Some(&target)),
                    ))
                }
            }
        };
        let mut digest = Sha256::new();
        let mut size = 0u64;
        let copied = (|| -> Result<(), (ExportErrorCode, &'static str, io::Error)> {
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let n = source
                    .read(&mut buffer)
                    .map_err(|error| (ExportErrorCode::SourceReadFailed, "read_source", error))?;
                if n == 0 {
                    break;
                }
                output
                    .write_all(&buffer[..n])
                    .map_err(|error| (code, "write_target", error))?;
                digest.update(&buffer[..n]);
                size += n as u64;
            }
            output
                .flush()
                .map_err(|error| (code, "flush_target", error))
        })();
        drop(output);
        if let Err((error_code, operation, error)) = copied {
            let _ = std::fs::remove_file(&target);
            return Err(self.failed(
                key,
                entry,
                ExportError::io(error_code, operation, error, Some(&target)),
            ));
        }
        let link = target.display().to_string();
        entry.path = Some(link.clone());
        entry.kind = "file";
        entry.size = Some(size);
        entry.sha256 = Some(format!("{:x}", digest.finalize()));
        entry.relative_link = target
            .file_name()
            .map(|name| relative_uri(&name.to_string_lossy()));
        self.record(key, entry);
        Ok(link)
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0'])
}
fn relative_uri(name: &str) -> String {
    let mut out = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
fn infer_type(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}
/// 取路径里的文件名（含扩展名）
pub fn file_name_of(src: &str) -> &str {
    src.rsplit(['/', '\\']).next().unwrap_or(src)
}

/// 拆「主干 / 扩展名（含点）」
pub fn split_ext(name: &str) -> (&str, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], name[i..].to_string()),
        _ => (name, String::new()),
    }
}

/// 判断是否 http(s) URL
pub fn is_url(src: &str) -> bool {
    src.starts_with("http://") || src.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn tmpdir(tag: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("office-core-attach-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn file_payload_is_copied_and_link_points_to_copy() {
        let base = tmpdir("file");
        let src = base.join("payload.xlsx");
        std::fs::write(&src, b"PK-payload").unwrap();
        let out = base.join("out");
        let sink = PayloadSink::new(&out);
        let link = sink.link_for("测试数据.xlsx", &src.display().to_string());
        assert_eq!(link, out.join("测试数据.xlsx").display().to_string());
        assert_eq!(
            std::fs::read(out.join("测试数据.xlsx")).unwrap(),
            b"PK-payload"
        );
        let e = &sink.entries()[0];
        assert_eq!((e.kind, e.name.as_str()), ("file", "测试数据.xlsx"));
        assert_eq!(e.src, src.display().to_string());
        // 同一 src 再引用一次：复用同一目标，且报告里只记一条（不重复导出/不重复报）
        assert_eq!(
            sink.link_for("测试数据.xlsx", &src.display().to_string()),
            link
        );
        assert_eq!(sink.entries().len(), 1);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn url_is_kept_as_is() {
        let sink = PayloadSink::new("/tmp/never-used-attach");
        let link = sink.link_for_url("远程.pdf", "https://example.com/远程.pdf");
        assert_eq!(link, "https://example.com/远程.pdf");
        assert_eq!(sink.entries()[0].kind, "url");
        assert!(sink.entries()[0].path.is_none());
    }

    #[test]
    fn missing_payload_keeps_original_reference() {
        let sink = PayloadSink::new(tmpdir("missing-out"));
        let link = sink.link_for("x.bin", "ppt/embeddings/x.bin");
        assert_eq!(link, "ppt/embeddings/x.bin");
        assert_eq!(sink.entries()[0].kind, "missing");
    }

    #[test]
    fn mid_stream_source_read_failure_is_not_reported_as_copy_failure() {
        struct FailsAfterOneChunk(bool);
        impl Read for FailsAfterOneChunk {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("injected source read failure"));
                }
                self.0 = true;
                buf[0] = b'x';
                Ok(1)
            }
        }

        let base = tmpdir("source-read-failure");
        let out = base.join("out");
        let sink = PayloadSink::new(&out);
        let key = "file:injected-source".to_string();
        let entry = sink.entry("broken.bin", "injected-source", &key, None);
        let error = sink
            .export(
                key,
                entry,
                &mut FailsAfterOneChunk(false),
                ExportErrorCode::CopyFailed,
            )
            .unwrap_err();

        assert_eq!(error.code, ExportErrorCode::SourceReadFailed);
        assert_eq!(error.operation, "read_source");
        assert!(!out.join("broken.bin").exists());
        assert_eq!(sink.entries()[0].kind, "error");
        assert_eq!(
            sink.entries()[0].error.as_ref().unwrap().code,
            ExportErrorCode::SourceReadFailed
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn existing_file_in_target_dir_is_not_overwritten() {
        let base = tmpdir("dedupe");
        let src = base.join("a.bin");
        std::fs::write(&src, b"new").unwrap();
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("a.bin"), b"pre-existing").unwrap();
        let sink = PayloadSink::new(&out);
        let link = sink.link_for("a.bin", &src.display().to_string());
        assert_eq!(link, out.join("a-2.bin").display().to_string());
        assert_eq!(std::fs::read(out.join("a.bin")).unwrap(), b"pre-existing");
        assert_eq!(std::fs::read(out.join("a-2.bin")).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn relative_src_resolves_against_root() {
        let base = tmpdir("root");
        let root = base.join("prod");
        std::fs::create_dir_all(root.join("word/embeddings")).unwrap();
        std::fs::write(root.join("word/embeddings/附件.docx"), b"payload").unwrap();
        let out = base.join("out");
        let sink = PayloadSink::new(&out).with_root(&root);
        let link = sink.link_for("附件.docx", "word/embeddings/附件.docx");
        assert_eq!(link, out.join("附件.docx").display().to_string());
        assert_eq!(std::fs::read(&link).unwrap(), b"payload");
        // 载荷不在 → 保留原引用
        let sink2 = PayloadSink::new(&out).with_root(&root);
        assert_eq!(
            sink2.link_for("无.docx", "word/embeddings/无.docx"),
            "word/embeddings/无.docx"
        );
        assert_eq!(sink2.entries()[0].kind, "missing");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn same_payload_via_relative_and_absolute_counts_once() {
        let base = tmpdir("dedupe-forms");
        let root = base.join("prod");
        std::fs::create_dir_all(root.join("ppt/media")).unwrap();
        std::fs::write(root.join("ppt/media/logo.png"), b"png").unwrap();
        let out = base.join("out");
        let sink = PayloadSink::new(&out).with_root(&root);
        // markdown 里是包内相对路径，media[] 里是绝对路径——同一文件，只该导出一份
        let a = sink.link_for("logo.png", "ppt/media/logo.png");
        let b = sink.link_for(
            "logo.png",
            &root.join("ppt/media/logo.png").display().to_string(),
        );
        assert_eq!(a, b, "两种写法应得到同一导出路径");
        assert_eq!(sink.entries().len(), 1, "只记一条");
        assert_eq!(std::fs::read(&a).unwrap(), b"png");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bytes_payload_is_written() {
        let out = tmpdir("bytes");
        let sink = PayloadSink::new(&out);
        let link = sink.link_for_bytes("ole1.bin", b"\x00\x01bytes", "ppt/embeddings/ole1.bin");
        assert_eq!(std::fs::read(&link).unwrap(), b"\x00\x01bytes");
        assert_eq!(sink.entries()[0].kind, "file");
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn manifest_has_stable_identity_hash_type_size_and_all_locations() {
        let base = super::tests::tmpdir("report-metadata");
        let root = base.join("product");
        std::fs::create_dir_all(root.join("word/media")).unwrap();
        let source = root.join("word/media/a.pdf");
        std::fs::write(&source, b"abc").unwrap();
        let sink = PayloadSink::new(base.join("out"))
            .with_root(&root)
            .with_source_document("source.docx")
            .with_strict(true);
        {
            let _scope = sink.at_source("part[1]/block[2]");
            sink.link_for("report #1.pdf", "word/media/a.pdf");
        }
        {
            let _scope = sink.at_source("part[2]/block[3]");
            sink.link_for("alias.pdf", source.to_str().unwrap());
        }
        let entries = sink.entries();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.size, Some(3));
        assert_eq!(
            entry.sha256.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(entry.resource_type, "application/pdf");
        assert_eq!(entry.relative_link.as_deref(), Some("report%20%231.pdf"));
        assert_eq!(
            entry.source_locations,
            ["/part[1]/block[2]", "/part[2]/block[3]"]
        );
        assert_eq!(entry.source_document.as_deref(), Some("source.docx"));
        assert!(sink.check().is_ok());
        let other = PayloadSink::new(base.join("other"))
            .with_root(&root)
            .with_source_document("source.docx");
        other.link_for("different.pdf", "word/media/a.pdf");
        assert_eq!(other.entries()[0].resource_id, entry.resource_id);
        let bytes = PayloadSink::new(base.join("bytes"));
        bytes
            .try_link_for_bytes_typed(
                "payload.bin",
                b"abc",
                "ppt/embeddings/ole.bin",
                Some("application/pdf"),
            )
            .unwrap();
        assert_eq!(bytes.entries()[0].sha256, entry.sha256);
        assert_eq!(bytes.entries()[0].resource_type, "application/pdf");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn failures_are_distinct_structured_cached_and_strict() {
        let base = super::tests::tmpdir("report-errors");
        let payload = base.join("payload.bin");
        std::fs::write(&payload, b"abc").unwrap();
        let blocked = base.join("not-a-directory");
        std::fs::write(&blocked, b"existing").unwrap();
        let sink = PayloadSink::new(blocked.join("out")).with_strict(true);
        let error = sink
            .try_link_for("a.bin", payload.to_str().unwrap())
            .unwrap_err();
        assert_eq!(error.code, ExportErrorCode::CreateDirFailed);
        assert!(error.io_kind.is_some() && error.target.is_some());
        assert_eq!(sink.entries()[0].kind, "error");
        assert_eq!(sink.entries()[0].size, Some(3));
        let missing = base.join("missing.bin");
        for location in ["block[1]", "block[2]"] {
            let _scope = sink.at_source(location);
            assert_eq!(
                sink.link_for("missing.bin", missing.to_str().unwrap()),
                missing.display().to_string()
            );
        }
        assert_eq!(sink.entries().len(), 2);
        assert_eq!(sink.entries()[1].kind, "missing");
        assert_eq!(
            sink.entries()[1].error.as_ref().unwrap().code,
            ExportErrorCode::PayloadMissing
        );
        assert_eq!(sink.entries()[1].source_locations.len(), 2);
        let failures = sink.check().unwrap_err();
        assert_eq!(failures.failures.len(), 2);
        let mut report = serde_json::json!({});
        assert!(PayloadSinks {
            attachments: Some(&sink),
            media: None
        }
        .annotate_report(&mut report));
        assert_eq!(report["ok"], false);
        assert_eq!(report["export_summary"]["failed"], 2);
        assert_eq!(report["error"]["code"], "resource_export_failed");
        let sink = PayloadSink::new(base.join("out"));
        let long_name = format!("{}.bin", "a".repeat(300));
        assert_eq!(
            sink.try_link_for(&long_name, payload.to_str().unwrap())
                .unwrap_err()
                .code,
            ExportErrorCode::CopyFailed
        );
        assert_eq!(
            sink.try_link_for_bytes(&long_name, b"x", "bytes")
                .unwrap_err()
                .code,
            ExportErrorCode::WriteFailed
        );
        assert_eq!(
            sink.try_link_for("dir.bin", base.to_str().unwrap())
                .unwrap_err()
                .code,
            ExportErrorCode::SourceReadFailed
        );
        assert!(
            sink.check().is_ok(),
            "tolerant batches keep their errors without failing the check"
        );
        let json = serde_json::to_value(sink.entries()).unwrap();
        assert_eq!(json[0]["error"]["code"], "copy_failed");
        assert_eq!(json[1]["error"]["code"], "write_failed");
        assert!(json[0]["relative_link"].is_null() && json[0]["sha256"].is_null());
        assert_eq!(std::fs::read(blocked).unwrap(), b"existing");
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn invalid_names_and_invalid_base64_are_reported_without_writes() {
        let base = super::tests::tmpdir("report-invalid");
        let sink = PayloadSink::new(base.join("out")).with_strict(true);
        for name in [
            "../escape.bin",
            "/absolute.bin",
            "a/b.bin",
            "a\\b.bin",
            "C:drive.bin",
            ".",
            "..",
            "",
        ] {
            assert_eq!(
                sink.try_link_for_bytes(name, b"x", name).unwrap_err().code,
                ExportErrorCode::InvalidName
            );
        }
        assert_eq!(
            sink.link_for_base64("broken.bin", "%%%", "ppt/embeddings/broken.bin", None),
            "ppt/embeddings/broken.bin"
        );
        assert_eq!(
            sink.entries().last().unwrap().error.as_ref().unwrap().code,
            ExportErrorCode::InvalidPayload
        );
        assert!(!sink.dir().exists());
        let url = sink.link_for_url("remote.pdf", "https://example.com/remote.pdf");
        assert_eq!(url, "https://example.com/remote.pdf");
        let entry = sink.entries().last().unwrap().clone();
        assert!(entry.error.is_none() && entry.size.is_none() && entry.sha256.is_none());
        let empty = PayloadSink::new(base.join("empty")).with_strict(true);
        empty.link_for_bytes("empty.bin", b"", "empty");
        assert_eq!(empty.entries()[0].size, Some(0));
        assert_eq!(
            empty.entries()[0].sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn partial_copy_is_removed_on_read_failure() {
        struct Broken(bool);
        impl Read for Broken {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("source failed mid-copy"));
                }
                self.0 = true;
                buffer[..3].copy_from_slice(b"abc");
                Ok(3)
            }
        }
        let base = super::tests::tmpdir("report-partial");
        let sink = PayloadSink::new(&base);
        let entry = sink.entry("partial.bin", "source", "source", None);
        let error = sink
            .export(
                "source".into(),
                entry,
                &mut Broken(false),
                ExportErrorCode::CopyFailed,
            )
            .unwrap_err();
        assert_eq!(error.code, ExportErrorCode::SourceReadFailed);
        assert!(!base.join("partial.bin").exists());
        assert!(sink.entries()[0].sha256.is_none());
        std::fs::remove_dir_all(base).unwrap();
    }
}
