//! LibreOffice（soffice）无头转换引擎：旧格式 → 新格式的最高保真路径。
//!
//! 纯 Rust 转换器（rwml / calamine / office_oxide）对样式的覆盖有限（见
//! docs/legacy-formats.md），LibreOffice 的旧格式导入是最完整的可用实现。
//! 本模块提供探测与无头转换：`--engine auto` 时检测到 soffice 即用它，
//! 找不到则调用方回退纯 Rust，二者都不构成硬依赖。

use crate::error::CliError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// 常见安装位置 + PATH 探测 soffice
///
/// `ASTORM_SOFFICE_PATH` 环境变量最优先（桌面端可指定自带/非标准安装路径）。
pub fn find_soffice() -> Option<PathBuf> {
    if let Ok(bin) = std::env::var("ASTORM_SOFFICE_PATH") {
        let p = PathBuf::from(&bin);
        if p.is_file() {
            return Some(p);
        }
    }
    let absolute = [
        "/opt/homebrew/bin/soffice",
        "/usr/local/bin/soffice",
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "C:/Program Files/LibreOffice/program/soffice.exe",
    ];
    for c in absolute {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    // PATH 探测：soffice --version 能执行即视为存在
    let ok = Command::new("soffice")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| PathBuf::from("soffice"))
}

/// soffice 是否可用（`--engine auto` 的判据）
pub fn available() -> bool {
    find_soffice().is_some()
}

/// 用 soffice 无头把 input 转成 target 格式（"docx" / "xlsx" / "pptx"），写到 out。
/// 使用独立 UserInstallation profile，避免与正在运行的 LibreOffice 实例抢锁。
pub fn convert(input: &Path, target: &str, out: &Path) -> Result<(), CliError> {
    let soffice = find_soffice().ok_or_else(|| {
        CliError::new(
            "--engine libreoffice 需要 LibreOffice，但未找到 soffice（PATH 与常见安装位置均无）"
                .to_string(),
        )
        .suggest("安装 LibreOffice，或改用 --engine rust（保真度较低）")
    })?;

    // 独立临时目录：产出 + profile（每次唯一，避免并发与残留锁）
    let dir = std::env::temp_dir().join(format!(
        "soffice-conv-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| CliError::new(format!("创建临时目录失败: {e}")))?;

    let profile = dir.join("profile");
    let mut child = Command::new(&soffice)
        .arg("--headless")
        .arg(format!(
            "-env:UserInstallation=file://{}",
            profile.display()
        ))
        .arg("--convert-to")
        .arg(target)
        .arg("--outdir")
        .arg(&dir)
        .arg(input)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| CliError::new(format!("启动 soffice 失败: {e}")))?;

    // 轮询等待：部分版本完成后进程不退出，超时 kill（chrome.rs 同法）
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let _ = std::fs::remove_dir_all(&dir);
                    return Err(CliError::new(format!(
                        "soffice 转换失败（exit {status}）: {} → {target}",
                        input.display()
                    )));
                }
                break;
            }
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = std::fs::remove_dir_all(&dir);
                    return Err(CliError::new(format!(
                        "soffice 转换超时（180s）: {}",
                        input.display()
                    )));
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(CliError::new(format!("等待 soffice 失败: {e}"))),
        }
    }

    // 产物名 = 输入 stem + 目标扩展名
    let stem = input
        .file_stem()
        .ok_or_else(|| CliError::new("输入路径缺少文件名"))?;
    let produced = dir.join(format!("{}.{}", stem.to_string_lossy(), target));
    if !produced.exists() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(CliError::new(format!(
            "soffice 未产出 {} 文件: {}",
            target,
            produced.display()
        )));
    }
    if out.exists() {
        std::fs::remove_file(out).map_err(|e| CliError::new(format!("覆盖旧产物失败: {e}")))?;
    }
    // rename 跨卷会失败，退回 copy
    if std::fs::rename(&produced, out).is_err() {
        std::fs::copy(&produced, out).map_err(|e| CliError::new(format!("写入产物失败: {e}")))?;
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

/// 把 OOXML 包内浏览器无法显示的 WMF/EMF 图片光栅化为 PNG：
/// 逐个用 soffice 转 png，写回包内新部件，并把 .rels 的 Target 改指到 .png。
/// 转换失败的图片保留原样。返回成功光栅化的数量。
pub fn rasterize_metafiles(pkg: &Path) -> Result<usize, CliError> {
    use crate::opc;
    let names = opc::list_parts(pkg).map_err(|e| CliError::new(format!("读取包失败: {e}")))?;
    let metas: Vec<String> = names
        .iter()
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            lower.contains("/media/") && (lower.ends_with(".wmf") || lower.ends_with(".emf"))
        })
        .cloned()
        .collect();
    if metas.is_empty() {
        return Ok(0);
    }

    let dir = std::env::temp_dir().join(format!(
        "soffice-ras-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| CliError::new(format!("创建临时目录失败: {e}")))?;

    let mut done = 0usize;
    for meta in &metas {
        // 提取元文件到临时目录（保留扩展名供 soffice 识别）
        let Some(bytes) = opc::read_part(pkg, meta).ok().flatten() else {
            continue;
        };
        let stem = format!("m{}", done);
        let ext = meta
            .rsplit('.')
            .next()
            .unwrap_or("wmf")
            .to_ascii_lowercase();
        let tmp_in = dir.join(format!("{stem}.{ext}"));
        std::fs::write(&tmp_in, &bytes)
            .map_err(|e| CliError::new(format!("写临时文件失败: {e}")))?;
        let tmp_png = dir.join(format!("{stem}.png"));
        if convert(&tmp_in, "png", &tmp_png).is_err() {
            continue; // 个别元文件转换失败：保留原样
        }
        let png =
            std::fs::read(&tmp_png).map_err(|e| CliError::new(format!("读取 PNG 失败: {e}")))?;
        let png_part = format!("{}.png", &meta[..meta.len() - ext.len() - 1]);

        // 写入 PNG 部件（整包重写到临时文件再替换）
        let tmp_pkg = dir.join("out.pkgx");
        opc::write_part(pkg, &tmp_pkg, &png_part, &png)
            .map_err(|e| CliError::new(format!("写回包失败: {e}")))?;
        std::fs::rename(&tmp_pkg, pkg).map_err(|e| CliError::new(format!("替换包失败: {e}")))?;

        // 改 .rels：Target 里的元文件名 → .png（文件名在包内约定唯一）
        let old_name = meta.rsplit('/').next().unwrap_or_default().to_string();
        let new_name = png_part.rsplit('/').next().unwrap_or_default().to_string();
        if !old_name.is_empty() {
            for rel in names.iter().filter(|n| n.ends_with(".rels")) {
                if let Ok(Some(xml)) = opc::read_part(pkg, rel) {
                    let text = String::from_utf8_lossy(&xml).to_string();
                    if text.contains(&old_name) {
                        let updated = text.replace(&old_name, &new_name);
                        let tmp_pkg = dir.join("out.pkgx");
                        if opc::write_part(pkg, &tmp_pkg, rel, updated.as_bytes()).is_ok() {
                            let _ = std::fs::rename(&tmp_pkg, pkg);
                        }
                    }
                }
            }
        }
        // [Content_Types].xml 确保 png 有 Default 声明
        if let Ok(Some(xml)) = opc::read_part(pkg, "[Content_Types].xml") {
            let text = String::from_utf8_lossy(&xml).to_string();
            if !text.contains("Extension=\"png\"") {
                let updated = text.replace(
                    "</Types>",
                    "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
                );
                let tmp_pkg = dir.join("out.pkgx");
                if opc::write_part(pkg, &tmp_pkg, "[Content_Types].xml", updated.as_bytes()).is_ok()
                {
                    let _ = std::fs::rename(&tmp_pkg, pkg);
                }
            }
        }
        done += 1;
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(done)
}
