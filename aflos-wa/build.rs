//! Gives the helper Aflos's icon and a proper name in Task Manager.

fn main() {
    println!("cargo:rerun-if-changed=../crates/aflos/assets/aflos.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../crates/aflos/assets/aflos.ico")
            .set("ProductName", "Aflos")
            .set("FileDescription", "Aflos WhatsApp")
            .set("CompanyName", "Aflos")
            .set("OriginalFilename", "aflos-wa.exe");
        if let Err(err) = res.compile() {
            println!("cargo:warning=no exe icon: {err}");
        }
    }
}
