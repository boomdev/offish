fn main() {
    #[cfg(feature = "desktop")]
    {
        let bundle = std::path::Path::new("web/editor.js");
        println!("cargo:rerun-if-changed=web");
        assert!(
            bundle
                .metadata()
                .is_ok_and(|file| file.is_file() && file.len() > 0),
            "The desktop editor bundle is missing. Run `npm --prefix apps/desktop run build` from the repository root."
        );
        tauri_build::build();
    }
}
