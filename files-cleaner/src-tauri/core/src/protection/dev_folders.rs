//! Developer folders: version control, dependencies, build output, caches
//! and tool state for common languages and tools. Moving or trashing these
//! breaks projects, and they hold huge numbers of files, so they are
//! Protected Paths. None of this is OS-specific.

use std::fs;
use std::path::Path;

/// Names that mean a developer folder wherever they appear.
const ALWAYS: &[&str] = &[
    // Version control
    ".git", ".hg", ".svn", ".bzr", "_darcs", ".jj", ".pijul",
    // JavaScript and TypeScript
    "node_modules", "bower_components", "jspm_packages", ".pnpm-store", ".yarn", ".npm",
    ".next", ".nuxt", ".output", ".svelte-kit", ".angular", ".astro", ".vite", ".parcel-cache",
    ".turbo", ".vercel", ".netlify", ".expo", ".docusaurus", ".nx", ".eslintcache",
    // Python
    "__pycache__", "__pypackages__", "site-packages", ".venv", ".tox", ".nox", ".mypy_cache",
    ".pytest_cache", ".ruff_cache", ".pytype", ".pyre", ".hypothesis", ".ipynb_checkpoints",
    ".eggs", ".pdm-build", ".direnv", "htmlcov",
    // Rust
    ".cargo", ".rustup",
    // JVM: Java, Kotlin, Scala, Clojure
    ".gradle", ".m2", ".mvn", ".kotlin", ".bsp", ".metals", ".bloop", ".cpcache", ".lsp",
    // Apple: Swift, Xcode, CocoaPods
    "DerivedData", "xcuserdata", ".build", ".swiftpm",
    // Android and native builds
    ".cxx", ".externalNativeBuild", "CMakeFiles", ".ccls-cache", ".clangd",
    // Haskell, OCaml, Elm, Zig, Dart, Ruby, R
    ".stack-work", "dist-newstyle", "_opam", "elm-stuff", "zig-cache", ".zig-cache",
    ".dart_tool", ".pub-cache", ".bundle", ".Rproj.user",
    // Infrastructure and static sites
    ".terraform", ".serverless", ".aws-sam", ".jekyll-cache",
    // Editors and IDEs
    ".idea", ".vscode", ".vs", ".fleet",
];

/// Name endings and beginnings that mean a developer folder.
const ALWAYS_SUFFIXES: &[&str] = &[".egg-info", ".dist-info"];
const ALWAYS_PREFIXES: &[&str] = &["cmake-build-"];

/// Files that, found inside any folder, say what it is whatever it is
/// called: a Python or Conda environment, or a Chrome/Chromium profile
/// folder (as browser automation tools create).
const CONTENT_MARKERS: &[(&str, &str)] = &[
    ("pyvenv.cfg", "a Python environment"),
    ("conda-meta", "a Conda environment"),
    ("Local State", "a Chrome or Chromium profile folder"),
];

/// Names that are too common to protect everywhere (a `build` folder of
/// photos is fine to clean), so they are developer folders only next to a
/// project file that creates them.
const NEXT_TO_FILE: &[(&str, &[&str])] = &[
    ("target", &["Cargo.toml", "pom.xml", "build.sbt", "project.clj"]),
    (
        "build",
        &[
            "package.json", "build.gradle", "build.gradle.kts", "settings.gradle",
            "settings.gradle.kts", "CMakeLists.txt", "meson.build", "setup.py", "pyproject.toml",
            "pubspec.yaml",
        ],
    ),
    ("dist", &["package.json", "setup.py", "setup.cfg", "pyproject.toml"]),
    ("out", &["package.json", "build.gradle", "build.gradle.kts", "CMakeLists.txt"]),
    ("coverage", &["package.json"]),
    ("vendor", &["go.mod", "composer.json", "Gemfile"]),
    ("Pods", &["Podfile"]),
    ("Carthage", &["Cartfile"]),
    ("_build", &["mix.exs", "rebar.config", "dune-project"]),
    ("deps", &["mix.exs", "rebar.config"]),
    ("zig-out", &["build.zig"]),
    ("_site", &["_config.yml"]),
    // Unity
    ("Library", &["ProjectSettings/ProjectVersion.txt"]),
    ("Temp", &["ProjectSettings/ProjectVersion.txt"]),
];

/// Like `NEXT_TO_FILE`, but the project file is known by its extension.
const NEXT_TO_EXTENSION: &[(&str, &[&str])] = &[
    // .NET
    ("bin", &["csproj", "fsproj", "vbproj", "sln"]),
    ("obj", &["csproj", "fsproj", "vbproj", "sln"]),
    // Unreal Engine
    ("Binaries", &["uproject", "uplugin"]),
    ("Intermediate", &["uproject", "uplugin"]),
    ("DerivedDataCache", &["uproject"]),
    ("Saved", &["uproject"]),
];

/// Why `path` is a developer folder, if it is. `is_folder` says whether
/// `path` is a folder, so a plain file is never checked for a marker inside.
pub fn reason(path: &Path, is_folder: bool) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let lower = name.to_lowercase();
    let is = |want: &str| name.eq_ignore_ascii_case(want);

    if ALWAYS.iter().any(|n| is(n))
        || ALWAYS_SUFFIXES.iter().any(|s| lower.ends_with(s))
        || ALWAYS_PREFIXES.iter().any(|p| lower.starts_with(p))
    {
        return Some(format!("{name} is a developer folder"));
    }
    if !is_folder {
        return None;
    }
    if let Some((marker, what)) = CONTENT_MARKERS.iter().find(|(m, _)| path.join(m).exists()) {
        return Some(format!("{name} is {what} (it contains {marker})"));
    }
    let parent = path.parent()?;
    for (folder, files) in NEXT_TO_FILE {
        if !is(folder) {
            continue;
        }
        if let Some(file) = files.iter().find(|f| parent.join(f).exists()) {
            return Some(format!("{name} is a developer folder (it sits next to {file})"));
        }
    }
    for (folder, extensions) in NEXT_TO_EXTENSION {
        if is(folder) {
            if let Some(file) = file_with_extension(parent, extensions) {
                return Some(format!("{name} is a developer folder (it sits next to {file})"));
            }
        }
    }
    None
}

/// The name of a file in `folder` with one of `extensions`, if any.
fn file_with_extension(folder: &Path, extensions: &[&str]) -> Option<String> {
    fs::read_dir(folder).ok()?.flatten().find_map(|child| {
        let path = child.path();
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        extensions
            .contains(&extension.as_str())
            .then(|| child.file_name().to_string_lossy().into_owned())
    })
}
