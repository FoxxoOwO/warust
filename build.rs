fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("ProductName", "Warust");
        res.set("FileDescription", "Warust WhatsApp Desktop Client");
        res.set("LegalCopyright", "Copyright © 2026 Warust Authors");

        if let Err(e) = res.compile() {
            println!("cargo::warning=embedding Windows resources failed: {e}");
        }

        // winres hands the resources over as `libresource.a` and expects the
        // linker to pull them in. With the GNU toolchain that archive member has
        // no referenced symbols, so nothing forces it into the image and the
        // final exe ends up with an EMPTY resource directory - the tray icon
        // then fails with ERROR_RESOURCE_NAME_NOT_FOUND (1812). Passing the
        // object file itself guarantees it is linked.
        #[cfg(target_env = "gnu")]
        {
            let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
            let object = std::path::Path::new(&out_dir).join("resource.o");
            if object.exists() {
                println!("cargo:rustc-link-arg={}", object.display());
            } else {
                println!(
                    "cargo::warning=resource object missing at {}; the tray icon will have no image",
                    object.display()
                );
            }
            println!("cargo:rerun-if-changed=assets/app-icon.ico");
        }
    }
}
