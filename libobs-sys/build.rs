extern crate bindgen;
extern crate cmake;

use bindgen::callbacks::{MacroParsingBehavior, ParseCallbacks};
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
struct FpMacroIgnore;
impl ParseCallbacks for FpMacroIgnore {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        match name {
            "FP_ZERO" | "FP_SUBNORMAL" | "FP_NORMAL" | "FP_INFINITE" | "FP_NAN" => {
                MacroParsingBehavior::Ignore
            }
            _ => MacroParsingBehavior::Default,
        }
    }
}

fn main() {
    let dst = cmake::Config::new("obs-studio")
        .build_target("libobs")
        .build();

    println!("cargo:rustc-link-lib=dylib=obs");
    println!("cargo:rustc-link-search=native={}/build/libobs", dst.display());

    let bindings = bindgen::Builder::default()
        .parse_callbacks(Box::new(FpMacroIgnore))
        .header("wrapper.h")
        .derive_default(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
