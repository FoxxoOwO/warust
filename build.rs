fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("ProductName", "Warust");
        res.set("FileDescription", "Warust WhatsApp Desktop Client");
        res.set("LegalCopyright", "Copyright © 2026 Warust Authors");
        let _ = res.compile();
    }
}
