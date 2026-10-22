//! Links clang's runtime library on macOS. llama.cpp's Metal code checks the
//! OS version with `@available`, which below the newest macOS it checks for
//! (the app supports 10.15) compiles to calls into that library, and rustc
//! links with `-nodefaultlibs`, so it isn't linked otherwise.

use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "macos" {
        return;
    }
    let output = Command::new("xcrun")
        .args(["clang", "--print-resource-dir"])
        .output();
    let resources = match output {
        Ok(output) if output.status.success() => String::from_utf8(output.stdout).unwrap(),
        _ => panic!(
            "couldn't ask clang where its runtime library is; install the Xcode command line tools"
        ),
    };
    let folder = Path::new(resources.trim()).join("lib").join("darwin");
    if !folder.join("libclang_rt.osx.a").is_file() {
        panic!("no libclang_rt.osx.a in {}", folder.display());
    }
    println!("cargo:rustc-link-search=native={}", folder.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
