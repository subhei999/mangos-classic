fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    let version = env!("CARGO_PKG_VERSION");
    let mut resource = winresource::WindowsResource::new();
    resource
        .set("FileDescription", "Rusty MaNGOS Launcher")
        .set("ProductName", "Rusty MaNGOS")
        .set("CompanyName", "Rusty MaNGOS")
        .set("InternalName", "RustyMangosLauncher")
        .set("OriginalFilename", "RustyMangosLauncher.exe")
        .set("LegalCopyright", "Rusty MaNGOS contributors")
        .set("ProductVersion", version)
        .set("FileVersion", version);

    if let Err(error) = resource.compile() {
        panic!("compile Windows launcher resource: {error}");
    }
}
