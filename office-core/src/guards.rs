//! 一次性临时文件/目录守卫：作用域结束自动清理。

use std::path::PathBuf;

/// 临时目录守卫：Drop 时删除整个目录。
pub struct TempGuard {
    pub path: PathBuf,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// 临时文件守卫：Drop 时删除文件。
pub struct FileGuard {
    pub path: PathBuf,
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
