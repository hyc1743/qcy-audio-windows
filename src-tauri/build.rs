fn main() {
    tauri_build::build();

    // Tauri's build script attaches the Windows Common-Controls v6 manifest (required by
    // comctl32.dll!TaskDialogIndirect in muda/tray-icon) via `rustc-link-arg-bins`, which
    // does not cover `--lib` unit-test harnesses.
    // - On MSVC: pass `/MANIFESTDEPENDENCY:...` (merges cleanly without duplicate .rsrc errors).
    // - On GNU: attach Tauri's generated `libresource.a` so `--lib` test binaries get v6 manifest.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" {
        if target_env == "msvc" {
            println!(
                "cargo::rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
            );
        } else if target_env == "gnu" {
            if let Ok(out_dir) = std::env::var("OUT_DIR") {
                let gnu_res = std::path::PathBuf::from(out_dir).join("libresource.a");
                if gnu_res.exists() {
                    println!("cargo::rustc-link-arg=-Wl,--whole-archive");
                    println!("cargo::rustc-link-arg={}", gnu_res.display());
                    println!("cargo::rustc-link-arg=-Wl,--no-whole-archive");
                }
            }
        }
    }
}
