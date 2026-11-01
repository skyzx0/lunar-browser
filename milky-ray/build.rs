// Gives the Windows program Milky Ray's icon and name.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Lunar Browser Milky Ray");
        res.set("FileDescription", "Lunar Browser Milky Ray");
        res.compile().expect("Windows resources");
    }
}
