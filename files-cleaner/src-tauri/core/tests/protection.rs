mod common;

use std::fs;
use std::path::Path;

use common::{config, empty_config, scan_root, write};
use files_cleaner_core::{scan, scan_root_warning, ScanResult};

fn names(result: &ScanResult) -> Vec<String> {
    let mut names: Vec<String> = result.entries.iter().map(|e| e.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn skips_dev_folders_and_counts_them() {
    let dir = scan_root();
    write(dir.path(), "keep.txt", "k");
    write(dir.path(), "project/.git/HEAD", "ref");
    write(dir.path(), "project/node_modules/left-pad/index.js", "js");
    write(dir.path(), "project/.venv/bin/python", "py");
    write(dir.path(), "project/src/__pycache__/mod.pyc", "pyc");
    write(dir.path(), "rusty/Cargo.toml", "[package]");
    write(dir.path(), "rusty/target/debug/app", "bin");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["Cargo.toml", "keep.txt"]);
    assert_eq!(result.skipped, 5);
}

#[test]
fn skips_developer_folders_for_many_languages() {
    let dir = scan_root();
    write(dir.path(), "keep.txt", "k");
    for folder in [
        "py/.tox",
        "py/.mypy_cache",
        "py/.pytest_cache",
        "py/mylib.egg-info",
        "py/htmlcov",
        "web/.next",
        "web/.svelte-kit",
        "web/bower_components",
        "apple/DerivedData",
        "apple/.build",
        "hs/.stack-work",
        "hs/dist-newstyle",
        "dart/.dart_tool",
        "zig/zig-cache",
        "c/CMakeFiles",
        "c/cmake-build-debug",
        "infra/.terraform",
        "ide/.idea",
        "vcs/.hg",
    ] {
        write(dir.path(), &format!("{folder}/inside.txt"), "x");
    }

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["keep.txt"]);
    assert_eq!(result.skipped, 19);
}

#[test]
fn skips_environments_and_browser_profiles_whatever_they_are_called() {
    let dir = scan_root();
    write(dir.path(), "project/main.py", "print()");
    write(dir.path(), "project/my-env/pyvenv.cfg", "home = /usr/bin");
    write(dir.path(), "project/my-env/lib/site.py", "py");
    write(
        dir.path(),
        "conda/envs/science/conda-meta/history.json",
        "{}",
    );
    write(dir.path(), "scraper/google_profile/Local State", "{}");
    write(
        dir.path(),
        "scraper/google_profile/Default/History.db",
        "db",
    );

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["main.py"]);
    assert_eq!(result.skipped, 3);
}

#[test]
fn common_build_folder_names_are_skipped_only_next_to_a_project_file() {
    let dir = scan_root();
    write(dir.path(), "web/package.json", "{}");
    write(dir.path(), "web/dist/app.js", "js");
    write(dir.path(), "web/build/app.js", "js");
    write(dir.path(), "go/go.mod", "module x");
    write(dir.path(), "go/vendor/lib.go", "go");
    write(dir.path(), "java/pom.xml", "<project/>");
    write(dir.path(), "java/target/app.jar", "jar");
    write(dir.path(), "dotnet/App.csproj", "<Project/>");
    write(dir.path(), "dotnet/bin/App.dll", "dll");
    write(dir.path(), "dotnet/obj/App.pdb", "pdb");
    write(dir.path(), "photos/build/house.jpg", "jpg");
    write(dir.path(), "photos/dist/plan.pdf", "pdf");
    write(dir.path(), "photos/vendor/invoice.pdf", "pdf");
    write(dir.path(), "photos/bin/old.zip", "zip");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(
        names(&result),
        [
            "App.csproj",
            "go.mod",
            "house.jpg",
            "invoice.pdf",
            "old.zip",
            "package.json",
            "plan.pdf",
            "pom.xml"
        ]
    );
    assert_eq!(result.skipped, 6);
}

#[test]
fn warns_about_a_python_environment_as_the_scan_root() {
    let home = scan_root();
    write(home.path(), "env/pyvenv.cfg", "home = /usr/bin");
    write(home.path(), "env/lib/site.py", "py");

    let reason = warning(&home.path().join("env/lib"), home.path());

    assert!(reason.contains("pyvenv.cfg"), "{reason}");
}

#[test]
fn a_target_folder_without_cargo_toml_is_ordinary() {
    let dir = scan_root();
    write(dir.path(), "marketing/target/audience.txt", "a");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["audience.txt"]);
    assert_eq!(result.skipped, 0);
}

#[test]
fn skips_os_metadata_files_from_every_os() {
    let dir = scan_root();
    write(dir.path(), "photo.jpg", "jpg");
    write(dir.path(), ".DS_Store", "ds");
    write(dir.path(), "sub/.localized", "");
    // Windows doesn't allow a carriage return in a file name.
    #[cfg(not(windows))]
    write(dir.path(), "sub/Icon\r", "icon");
    write(dir.path(), "sub/Thumbs.db", "thumbs");
    write(dir.path(), "sub/desktop.ini", "ini");
    write(dir.path(), "sub/deeper/.directory", "kde");
    write(dir.path(), "sub/deeper/THUMBS.DB", "thumbs");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["photo.jpg"]);
    assert_eq!(result.skipped, if cfg!(windows) { 6 } else { 7 });
}

#[test]
fn skips_app_libraries() {
    let dir = scan_root();
    write(dir.path(), "song.mp3", "mp3");
    write(
        dir.path(),
        "Pictures/Photos Library.photoslibrary/database/db",
        "db",
    );
    write(dir.path(), "Music/Music/Media/track.m4a", "m4a");
    write(dir.path(), "Music/iTunes/iTunes Library.xml", "xml");
    write(dir.path(), "Music/Old.musiclibrary/x", "x");
    write(dir.path(), "Movies/TV.tvlibrary/x", "x");
    write(dir.path(), "Lightroom/Catalog.lrcat", "cat");
    write(dir.path(), "Lightroom/Catalog Previews.lrdata/p", "p");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["song.mp3"]);
    assert_eq!(result.skipped, 7);
}

#[test]
fn skips_the_holding_folder() {
    let dir = scan_root();
    write(dir.path(), "new.pdf", "pdf");
    write(dir.path(), "Files Cleaner Trash/session-1/old.pdf", "pdf");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["new.pdf"]);
    assert_eq!(result.skipped, 1);
}

#[test]
#[cfg(target_os = "macos")]
fn skips_the_users_library_folder() {
    let home = scan_root();
    write(home.path(), "Library/Preferences/app.plist", "plist");
    write(home.path(), "Documents/Library/book.epub", "epub");

    let result = scan(home.path(), &empty_config(home.path())).unwrap();

    assert_eq!(names(&result), ["book.epub"]);
    assert_eq!(result.skipped, 1);
}

#[test]
fn skips_never_touch_paths_from_the_config() {
    let home = scan_root();
    write(home.path(), "Downloads/Keep/precious.zip", "zip");
    write(home.path(), "Downloads/Other/precious.zip", "zip");
    write(home.path(), "Downloads/Tax/2025.pdf", "pdf");
    let tax = fs::canonicalize(home.path()).unwrap().join("Downloads/Tax");
    let text = format!(
        r#"{{ "neverTouch": ["~/Downloads/Keep", {}], "rules": [] }}"#,
        serde_json::to_string(&tax).unwrap()
    );

    let result = scan(&home.path().join("Downloads"), &config(&text, home.path())).unwrap();

    assert_eq!(result.entries.len(), 1);
    assert!(result.entries[0].path.ends_with("Other/precious.zip"));
    assert_eq!(result.skipped, 2);
}

/// The warning shown before scanning `root`, which must have one.
fn warning(root: &Path, home: &Path) -> String {
    scan_root_warning(root, &empty_config(home))
        .unwrap_or_else(|| panic!("expected a warning about {}", root.display()))
}

#[test]
#[cfg(target_os = "macos")]
fn warns_about_system_folders_as_the_scan_root() {
    let home = scan_root();
    for root in [
        "/System",
        "/Library/Fonts",
        "/Applications",
        "/usr/bin",
        "/bin",
        "/sbin",
        "/private/etc",
        "/Volumes",
    ] {
        warning(Path::new(root), home.path());
    }
}

#[test]
#[cfg(target_os = "macos")]
fn warns_about_a_scan_root_that_reaches_a_system_folder_through_a_symlink() {
    // `/tmp` is a symlink to `/private/tmp`.
    let home = scan_root();
    warning(Path::new("/tmp"), home.path());
}

#[test]
#[cfg(target_os = "macos")]
fn warns_about_a_volume_root_as_the_scan_root() {
    let home = scan_root();
    let volume = fs::read_dir("/Volumes")
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    warning(&volume, home.path());
}

#[test]
fn warns_about_protected_folders_and_anything_inside_them_as_the_scan_root() {
    let home = scan_root();
    let project = home.path().join("project");
    write(&project, "node_modules/pkg/lib/index.js", "js");
    write(&project, "Cargo.toml", "[package]");
    write(&project, "target/debug/app", "bin");
    write(home.path(), "Library/Caches/x", "x");
    write(home.path(), "Tool.app/Contents/Info.plist", "plist");

    let reason = warning(&project.join("node_modules/pkg/lib"), home.path());
    assert!(reason.contains("node_modules"), "{reason}");
    warning(&project.join("target/debug"), home.path());
    warning(&home.path().join("Tool.app/Contents"), home.path());
}

#[test]
fn warns_about_a_never_touch_path_as_the_scan_root_but_scans_it() {
    let home = scan_root();
    write(home.path(), "Downloads/Keep/a.txt", "a");
    let config = config(
        r#"{ "neverTouch": ["~/Downloads/Keep"], "rules": [] }"#,
        home.path(),
    );
    let keep = home.path().join("Downloads/Keep");

    let reason = scan_root_warning(&keep, &config).unwrap();
    let result = scan(&keep, &config).unwrap();

    assert!(reason.contains("Never-Touch"), "{reason}");
    assert_eq!(names(&result), ["a.txt"]);
    assert_eq!(result.skipped, 0);
}

#[test]
fn an_ordinary_folder_has_no_warning() {
    let home = scan_root();
    write(home.path(), "Downloads/a.txt", "a");
    assert_eq!(
        scan_root_warning(&home.path().join("Downloads"), &empty_config(home.path())),
        None
    );
}

#[test]
#[cfg(target_os = "macos")]
fn a_folder_inside_the_library_folder_can_be_scanned_after_its_warning() {
    // Cloud drives such as OneDrive live in ~/Library/CloudStorage.
    let home = scan_root();
    let drive = home.path().join("Library/CloudStorage/OneDrive-Personal");
    write(&drive, "Reports/q3.pdf", "pdf");
    write(&drive, "photo.jpg", "jpg");

    let reason = warning(&drive, home.path());
    let result = scan(&drive, &empty_config(home.path())).unwrap();

    assert!(reason.contains("Library"), "{reason}");
    assert_eq!(names(&result), ["photo.jpg", "q3.pdf"]);
    assert_eq!(result.skipped, 0);
}

#[test]
fn protected_paths_further_inside_a_warned_scan_root_are_still_skipped() {
    let home = scan_root();
    let drive = home.path().join("Library/CloudStorage/OneDrive-Personal");
    write(&drive, "notes.txt", "n");
    write(&drive, "project/node_modules/left-pad/index.js", "js");
    write(&drive, "Photos.photoslibrary/database/Photos.sqlite", "db");
    let config = config(
        r#"{ "neverTouch": ["~/Library/CloudStorage/OneDrive-Personal/Private"], "rules": [] }"#,
        home.path(),
    );
    write(&drive, "Private/secret.txt", "s");

    let result = scan(&drive, &config).unwrap();

    assert_eq!(names(&result), ["notes.txt"]);
    assert_eq!(result.skipped, 3);
}

#[test]
fn a_folder_inside_a_package_can_be_scanned_after_its_warning() {
    let home = scan_root();
    write(home.path(), "Tool.app/Contents/Info.plist", "plist");
    let contents = home.path().join("Tool.app/Contents");

    let reason = warning(&contents, home.path());
    let result = scan(&contents, &empty_config(home.path())).unwrap();

    assert!(reason.contains("Package"), "{reason}");
    assert_eq!(names(&result), ["Info.plist"]);
}

#[test]
#[cfg(unix)]
fn warns_about_the_startup_disk_root_as_the_scan_root() {
    let home = scan_root();
    warning(Path::new("/"), home.path());
}

#[test]
#[cfg(target_os = "macos")]
fn a_folder_on_a_disk_under_volumes_can_be_scanned() {
    // The startup disk also appears under /Volumes; reach the test folder
    // through it, as if it were on an external disk.
    let Some(disk) = fs::read_dir("/Volumes")
        .unwrap()
        .map(|d| d.unwrap().path())
        .find(|d| fs::canonicalize(d).is_ok_and(|real| real == Path::new("/")))
    else {
        return;
    };
    let dir = scan_root();
    write(dir.path(), "a.txt", "a");
    let real = fs::canonicalize(dir.path()).unwrap();
    let via_volumes = disk.join(real.strip_prefix("/").unwrap());

    let result = scan(&via_volumes, &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["a.txt"]);
}

#[test]
fn reasons_use_the_os_neutral_words() {
    let home = scan_root();
    write(home.path(), "Thumbs.db/inside.txt", "x");

    let reason = warning(&home.path().join("Thumbs.db"), home.path());

    assert!(reason.contains("is an OS Metadata File"), "{reason}");
}

#[test]
fn a_package_copied_from_a_mac_is_one_item_on_every_os() {
    let dir = scan_root();
    write(dir.path(), "Essay.pages/Index/Document.iwa", "doc");
    write(dir.path(), "Backup.sparsebundle/bands/0", "band");

    let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

    assert_eq!(names(&result), ["Backup.sparsebundle", "Essay.pages"]);
    assert!(result.entries.iter().all(|e| e.is_package));
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[test]
    fn the_users_library_folder_is_a_system_folder() {
        let home = scan_root();
        write(home.path(), "Library/Caches/x", "x");

        let reason = warning(&home.path().join("Library/Caches"), home.path());

        assert!(reason.contains("is a System Folder"), "{reason}");
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    #[test]
    fn warns_about_system_folders_as_the_scan_root() {
        let home = scan_root();
        for root in [
            "/usr",
            "/usr/share",
            "/etc",
            "/bin",
            "/sbin",
            "/lib",
            "/lib64",
            "/boot",
            "/proc",
            "/sys",
            "/dev",
            "/run",
            "/var/log",
            "/opt",
            "/snap",
        ] {
            let reason = warning(Path::new(root), home.path());
            assert!(reason.contains("is a System Folder"), "{root}: {reason}");
        }
    }

    #[test]
    fn the_config_local_and_cache_folders_in_home_are_system_folders() {
        let home = scan_root();
        write(home.path(), "Documents/a.txt", "a");
        for folder in [".config", ".local/share", ".cache", ".CONFIG"] {
            let reason = warning(&home.path().join(folder), home.path());
            assert!(reason.contains("is a System Folder"), "{folder}: {reason}");
        }
        assert_eq!(
            scan_root_warning(&home.path().join("Documents"), &empty_config(home.path())),
            None
        );
    }

    #[test]
    fn skips_system_folders_in_home_when_scanning_it() {
        let home = scan_root();
        write(home.path(), "notes.txt", "n");
        // Not hidden here, so only protection keeps them out.
        let config = config(r#"{ "neverTouch": [] }"#, &home.path().join("me"));
        write(home.path(), "me/report.pdf", "pdf");
        write(home.path(), "me/.config/app/settings.json", "{}");

        let result = scan(&home.path().join("me"), &config).unwrap();

        assert_eq!(names(&result), ["report.pdf"]);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn disk_roots_are_protected_but_folders_on_the_disk_are_not() {
        let home = scan_root();
        for root in [
            "/",
            "/mnt",
            "/mnt/usb",
            "/media",
            "/media/me",
            "/media/me/USB STICK",
            "/run/media/me/disk",
        ] {
            let reason = warning(Path::new(root), home.path());
            assert!(
                reason.contains("is a Disk Root") || reason.contains("System Folder"),
                "{root}: {reason}"
            );
        }
        for root in [
            "/mnt/usb/Photos",
            "/media/me/USB STICK/Photos",
            "/run/media/me/disk/Photos",
        ] {
            assert_eq!(
                scan_root_warning(Path::new(root), &empty_config(home.path())),
                None,
                "{root}"
            );
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    #[test]
    fn warns_about_system_folders_as_the_scan_root() {
        let home = scan_root();
        for root in [
            r"C:\Windows",
            r"C:\Windows\System32",
            r"C:\Program Files",
            r"C:\Program Files (x86)\App",
            r"C:\ProgramData",
            r"C:\$Recycle.Bin",
            r"C:\System Volume Information",
            r"c:\windows",
        ] {
            let reason = warning(Path::new(root), home.path());
            assert!(reason.contains("is a System Folder"), "{root}: {reason}");
        }
    }

    #[test]
    fn the_app_data_folder_in_home_is_a_system_folder() {
        let home = scan_root();
        write(home.path(), "AppData/Local/x.txt", "x");

        let reason = warning(&home.path().join(r"AppData\Local"), home.path());

        assert!(reason.contains("is a System Folder"), "{reason}");
    }

    #[test]
    fn drive_roots_are_disk_roots() {
        let home = scan_root();
        let reason = warning(Path::new(r"C:\"), home.path());
        assert!(reason.contains("is a Disk Root"), "{reason}");
    }

    #[test]
    fn skips_entries_with_the_hidden_or_system_attribute() {
        use std::os::windows::process::CommandExt;
        let dir = scan_root();
        write(dir.path(), "seen.txt", "s");
        let hidden = write(dir.path(), "hidden.txt", "h");
        let system = write(dir.path(), "system.txt", "s");
        for (path, flag) in [(&hidden, "+h"), (&system, "+s")] {
            let status = std::process::Command::new("attrib")
                .arg(flag)
                .arg(path)
                .creation_flags(0)
                .status()
                .unwrap();
            assert!(status.success());
        }

        let result = scan(dir.path(), &empty_config(dir.path())).unwrap();

        assert_eq!(names(&result), ["seen.txt"]);
    }
}
