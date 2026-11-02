fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "RingTool");
        res.set("FileDescription", "RingTool");
        res.compile().expect("Failed to embed icon.ico");
    }
}
