fn gui_setup() {
    println!("cargo::rerun-if-changed=ui/");
    slint_build::compile("ui/Windows.slint").unwrap();
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    gui_setup();
}
