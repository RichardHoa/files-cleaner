//! Puts the libraries the app ships with in place before Tauri bundles them:
//! PDFium for reading PDFs (a pinned prebuilt release), and on Windows the
//! Vulkan loader, so the app starts on a machine with no Vulkan driver and
//! groups on the CPU (ADR-0004).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// Must match the `pdfium_7881` feature of pdfium-render in the grouping crate.
const PDFIUM_RELEASE: &str = "chromium%2F7881";

/// (archive name, SHA-256) per target OS and architecture.
fn pdfium_archive(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    Some(match (os, arch) {
        ("macos", "aarch64") => (
            "mac-arm64",
            "52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40",
        ),
        ("macos", "x86_64") => (
            "mac-x64",
            "6dedf83990e0e3d6b7c93c9e7589c5a126b0ae14b7464d76120cff7a26afb18b",
        ),
        ("windows", "x86_64") => (
            "win-x64",
            "73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08",
        ),
        ("windows", "aarch64") => (
            "win-arm64",
            "d3035d4d2cacac6ecd1a2ece197a3d702a1b2a58466276b9f870b8cb278a9d84",
        ),
        ("linux", "x86_64") => (
            "linux-x64",
            "1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d",
        ),
        ("linux", "aarch64") => (
            "linux-arm64",
            "ee7f7b7d5468958336a818c1cd580bdd20972846b7377b13f9a923d92d1d4674",
        ),
        _ => return None,
    })
}

fn main() {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    if let Err(e) = fetch_pdfium(&manifest.join("pdfium"), &os, &arch) {
        panic!("couldn't put PDFium in place: {e}");
    }
    if os == "windows" {
        if let Err(e) = copy_vulkan_loader(&manifest.join("vulkan")) {
            panic!("couldn't put the Vulkan loader in place: {e}");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=pdfium");
    println!("cargo:rerun-if-changed=vulkan");
    tauri_build::build()
}

/// Downloads PDFium for the target into `folder` once, checked against its
/// pinned SHA-256: the library (and its licence), nothing else.
fn fetch_pdfium(folder: &Path, os: &str, arch: &str) -> Result<(), String> {
    let library = match os {
        "macos" => "libpdfium.dylib",
        "windows" => "pdfium.dll",
        _ => "libpdfium.so",
    };
    if folder.join(library).exists() {
        return Ok(());
    }
    let (name, sha256) =
        pdfium_archive(os, arch).ok_or_else(|| format!("no prebuilt PDFium for {os} {arch}"))?;
    let url =
        format!("https://github.com/bblanchon/pdfium-binaries/releases/download/{PDFIUM_RELEASE}/pdfium-{name}.tgz");
    let bytes = ureq::get(&url)
        .call()
        .and_then(|mut r| r.body_mut().read_to_vec())
        .map_err(|e| format!("{url}: {e}"))?;
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != sha256 {
        return Err(format!(
            "{url} has SHA-256 {actual}, not the pinned {sha256}"
        ));
    }
    let unpacked = PathBuf::from(env::var("OUT_DIR").unwrap()).join("pdfium");
    let _ = fs::remove_dir_all(&unpacked);
    fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    let archive = unpacked.join("pdfium.tgz");
    fs::write(&archive, bytes).map_err(|e| e.to_string())?;
    // `tar` ships with macOS, Linux and Windows 10+.
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&unpacked)
        .status();
    if !status.is_ok_and(|s| s.success()) {
        return Err(format!("tar couldn't unpack {}", archive.display()));
    }
    // The archives keep the Windows DLL in bin/, the others in lib/.
    let from = unpacked
        .join(if os == "windows" { "bin" } else { "lib" })
        .join(library);
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    fs::copy(unpacked.join("LICENSE"), folder.join("PDFium LICENSE")).map_err(|e| e.to_string())?;
    fs::copy(&from, folder.join(library)).map_err(|e| format!("{}: {e}", from.display()))?;
    Ok(())
}

/// Copies `vulkan-1.dll` from the build machine into `folder`, to be
/// installed next to the app's executable. Recent Vulkan SDKs leave it out
/// of `Bin`: the Vulkan Runtime, which GPU drivers also install, puts it in
/// System32.
fn copy_vulkan_loader(folder: &Path) -> Result<(), String> {
    let to = folder.join("vulkan-1.dll");
    if to.exists() {
        return Ok(());
    }
    let sdk = env::var("VULKAN_SDK")
        .map_err(|_| "VULKAN_SDK isn't set; install the Vulkan SDK".to_string())?;
    let system32 = Path::new(&env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string()))
        .join("System32");
    let candidates = [
        Path::new(&sdk).join("Bin"),
        Path::new(&sdk).join("runtime").join("x64"),
        system32,
    ]
    .map(|f| f.join("vulkan-1.dll"));
    let from = candidates
        .iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            let looked: Vec<String> = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            format!(
                "no vulkan-1.dll in {}; install the Vulkan Runtime",
                looked.join(", ")
            )
        })?;
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    fs::copy(from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
    Ok(())
}
