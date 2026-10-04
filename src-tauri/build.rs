fn main() {
    // headless fork 无 tauri-build，Windows 下手动把上游图标嵌入 exe 资源段
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .compile()
            .expect("failed to compile Windows resources (icon.ico)");
    }
}
