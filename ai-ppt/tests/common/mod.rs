use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_pptx_path() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("json2pptx_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!("test_{}.pptx", id))
}

pub fn create_dummy_image() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("json2pptx_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("test_img_{}.png", id));
    // Minimal valid 1x1 red PNG
    let png_data: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77,
        0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x36, 0x28,
        0x19, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND
        0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&path, png_data).unwrap();
    path
}

#[allow(unused_macros)]
macro_rules! assert_positions_eq {
    ($a:expr, $b:expr) => {
        assert!(
            ($a.x - $b.x).abs() < 0.01,
            "x mismatch: {} vs {}",
            $a.x,
            $b.x
        );
        assert!(
            ($a.y - $b.y).abs() < 0.01,
            "y mismatch: {} vs {}",
            $a.y,
            $b.y
        );
        assert!(
            ($a.w - $b.w).abs() < 0.01,
            "w mismatch: {} vs {}",
            $a.w,
            $b.w
        );
        assert!(
            ($a.h - $b.h).abs() < 0.01,
            "h mismatch: {} vs {}",
            $a.h,
            $b.h
        );
    };
}

#[allow(unused_imports)]
pub(crate) use assert_positions_eq;
