//! Gives the Windows exe Aflos's icon and a proper name in Explorer and Task Manager.

fn main() {
    println!("cargo:rerun-if-changed=assets/mesh.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/mesh.ico")
            .set("ProductName", "Aflos")
            .set("FileDescription", "Aflos")
            .set("CompanyName", "Aflos")
            .set("OriginalFilename", "Aflos.exe");
        if let Err(err) = res.compile() {
            println!("cargo:warning=no exe icon: {err}");
        }
    }
}
