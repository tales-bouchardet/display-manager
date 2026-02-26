fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("icon.ico"); // ícone do executável
    res.compile().unwrap();
}
