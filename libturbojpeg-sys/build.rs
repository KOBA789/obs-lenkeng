extern crate bindgen;
extern crate cmake;

use std::env;
use std::path::PathBuf;

fn main() {
    let dst = cmake::Config::new("libjpeg-turbo")
        // FIXME: adhoc
        .define("CMAKE_ASM_NASM_COMPILER", "/opt/brew/bin/nasm")
        .build_target("turbojpeg-static")
        .build();

    println!("cargo:rustc-link-lib=static=turbojpeg");
    println!("cargo:rustc-link-search=native={}/build", dst.display());

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
