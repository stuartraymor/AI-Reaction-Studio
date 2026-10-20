#[cfg(windows)]
fn main() -> std::io::Result<()> {
    winresource::WindowsResource::new()
        .set_icon("assets/icon.ico")
        .set("ProductName", "AI Reaction Studio")
        .set("FileDescription", "AI Reaction Studio development preview")
        .set("LegalCopyright", "AI Reaction Studio contributors; MIT OR Apache-2.0")
        .compile()
}
#[cfg(not(windows))]
fn main() {}
