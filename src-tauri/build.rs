fn main() {
    // 官方 SDK 静态编译，避免从不可信路径加载 DLL。
    cc::Build::new()
        .file("vendor/bridge.c")
        .include("vendor")
        .define("EVERYTHING_STATIC", None)
        .warnings(false)
        .compile("Everything");
    println!("cargo:rerun-if-changed=vendor/Everything.c");
    println!("cargo:rerun-if-changed=vendor/bridge.c");
    println!("cargo:rerun-if-changed=vendor/Everything.h");
    println!("cargo:rerun-if-changed=vendor/Everything_IPC.h");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=advapi32");
    tauri_build::build();
}
