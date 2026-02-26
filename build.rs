fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for framework in
            ["AppKit", "CoreGraphics", "CoreFoundation", "ApplicationServices", "Accessibility"]
        {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
}
