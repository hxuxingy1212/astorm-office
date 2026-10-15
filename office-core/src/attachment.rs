//! 载荷导出：`view text --attachments/--media <DIR>` 用（附件与媒体共用同一出口）。
//!
//! 各格式模型里"附件"的载荷形态不同：docx/xlsx 是**文件路径**（包内相对路径
//! 或绝对路径/URL），pptx 的 `opaque.parts` 直接把字节以 base64 存在模型里。
//! 这里提供统一出口：把载荷落到用户指定目录，并给出 Markdown 里该用的链接目标，
//! 同时记录每条导出（供 CLI 的 stderr 报告与 `--json` 信封）。

use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 视图导出的两个出口（都可选）：`--attachments` 与 `--media`
#[derive(Clone, Copy, Default)]
pub struct PayloadSinks<'a> {
    pub attachments: Option<&'a PayloadSink>,
    pub media: Option<&'a PayloadSink>,
}

/// 一条导出记录
#[derive(Debug, Clone, Serialize)]
pub struct ExportedAttachment {
    /// 展示名（附件自带的名字，否则取载荷文件名）
    pub name: String,
    /// 原始引用（包内相对路径 / 绝对路径 / URL）
    pub src: String,
    /// 导出后的位置；URL 与找不到载荷时为 None
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// `file`（已导出）/ `url`（原样保留）/ `missing`（载荷不在）
    pub kind: &'static str,
}

/// 载荷导出器（附件与媒体共用）。**只读**使用：内部用 `RefCell` 记录，所以 markdown 写出器
/// 可以按 `&PayloadSink` 共享传递（不用一路 `&mut`）。
pub struct PayloadSink {
    /// 包内相对路径的解析基准（产物根）；不给则按当前工作目录解析
    root: Option<PathBuf>,
    dir: PathBuf,
    /// 同一次运行里 src → 已导出目标（同一附件多处引用只导出一次）
    cache: RefCell<HashMap<String, String>>,
    entries: RefCell<Vec<ExportedAttachment>>,
}

impl PayloadSink {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            root: None,
            dir: dir.into(),
            cache: RefCell::new(HashMap::new()),
            entries: RefCell::new(Vec::new()),
        }
    }

    /// 指定包内相对路径的解析基准（产物根）
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// 导出目录（原样，即用户在命令行里给的那个）
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 载荷路径：绝对路径原样；相对路径按产物根解析（没给根则按当前工作目录）
    fn payload_path(&self, src_label: &str) -> PathBuf {
        let p = Path::new(src_label);
        if p.is_absolute() {
            return p.to_path_buf();
        }
        match &self.root {
            Some(root) => root.join(p),
            None => p.to_path_buf(),
        }
    }

    /// 本次导出记录（按遇到顺序）
    pub fn entries(&self) -> Vec<ExportedAttachment> {
        self.entries.borrow().clone()
    }

    /// URL 形态：不导出，原样保留（"保留 url"）
    pub fn link_for_url(&self, name: &str, src: &str) -> String {
        if let Some(hit) = self.cache.borrow().get(src) {
            return hit.clone();
        }
        self.cache
            .borrow_mut()
            .insert(src.to_string(), src.to_string());
        self.record(ExportedAttachment {
            name: name.to_string(),
            src: src.to_string(),
            path: None,
            kind: "url",
        });
        src.to_string()
    }

    /// 文件形态：`src_label` 是模型里的原始引用（包内相对路径 / 绝对路径）。
    ///
    /// 去重键是**解析后的载荷路径**——同一个文件在模型里可能以相对路径出现
    /// （markdown 里）又以绝对路径出现（`media[]` 里），不能导出两份。
    pub fn link_for(&self, name: &str, src_label: &str) -> String {
        let payload = self.payload_path(src_label);
        let payload = payload.as_path();
        if !payload.is_file() {
            return self.missing(name, src_label);
        }
        // 键用 canonicalize 后的路径：`/tmp/x` 与 `/private/tmp/x`（macOS 软链）是同一个文件，
        // 模型里相对路径与 media[] 的绝对路径也必须归一到同一个键
        let key = std::fs::canonicalize(payload)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| payload.to_string_lossy().to_string());
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.clone();
        }
        let target = self.unique_target(name);
        if std::fs::create_dir_all(&self.dir).is_err() || std::fs::copy(payload, &target).is_err() {
            return self.missing(name, src_label);
        }
        self.finish(name, src_label, target, key)
    }

    /// 字节形态（pptx 的 opaque 部件把载荷存在模型里）
    pub fn link_for_bytes(&self, name: &str, bytes: &[u8], src_label: &str) -> String {
        if let Some(hit) = self.cache.borrow().get(src_label) {
            return hit.clone();
        }
        let target = self.unique_target(name);
        if std::fs::create_dir_all(&self.dir).is_err() || std::fs::write(&target, bytes).is_err() {
            return self.missing(name, src_label);
        }
        self.finish(name, src_label, target, src_label.to_string())
    }

    fn finish(&self, name: &str, src_label: &str, target: PathBuf, key: String) -> String {
        let link = target.display().to_string();
        self.cache.borrow_mut().insert(key, link.clone());
        self.record(ExportedAttachment {
            name: name.to_string(),
            src: src_label.to_string(),
            path: Some(link.clone()),
            kind: "file",
        });
        link
    }

    fn missing(&self, name: &str, src_label: &str) -> String {
        self.record(ExportedAttachment {
            name: name.to_string(),
            src: src_label.to_string(),
            path: None,
            kind: "missing",
        });
        src_label.to_string()
    }

    /// 目标目录里不覆盖已有文件：同名则 `名字-2.ext`、`名字-3.ext`…
    fn unique_target(&self, name: &str) -> PathBuf {
        let (stem, ext) = split_ext(name);
        let mut cand = self.dir.join(name);
        let mut n = 2;
        while cand.exists() {
            cand = self.dir.join(format!("{stem}-{n}{ext}"));
            n += 1;
        }
        cand
    }

    fn record(&self, e: ExportedAttachment) {
        self.entries.borrow_mut().push(e);
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

    fn tmpdir(tag: &str) -> PathBuf {
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
