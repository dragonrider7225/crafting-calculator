fn main() {
    #[cfg(feature = "gui")]
    slint_build::compile("ui/MainWindow.slint").unwrap();
}
