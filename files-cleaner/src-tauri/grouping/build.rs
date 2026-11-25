//! llama.cpp's `@available` checks call clang's runtime library when targeting
//! older macOS, and rustc's `-nodefaultlibs` leaves it out.

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
