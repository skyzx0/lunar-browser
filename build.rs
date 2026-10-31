// Gives the Windows program Lunar's icon and name.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Lunar Browser");
        res.set("FileDescription", "Lunar Browser");
        res.compile().expect("Windows resources");
    }
}
