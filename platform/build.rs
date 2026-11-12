fn main() {
    println!("cargo:rerun-if-changed=native");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "macos" => {
            cc::Build::new()
                .file("native/macos.m")
                .flag("-fno-objc-arc")
                .flag("-Wno-deprecated-declarations")
                .compile("lanchat_macos");
            println!("cargo:rustc-link-lib=framework=Cocoa");
        }
        "windows" => {
            cc::Build::new()
                .file("native/windows.c")
                .compile("lanchat_windows");
            println!("cargo:rustc-link-lib=user32");
        }
        _ => {}
    }
}
