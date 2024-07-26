fn main() {
    #[cfg(feature = "gui")]
    slint_build::compile("ui/Windows.slint").unwrap();
}
