//! Tests of the temp folder of `install.sh`.

#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Runs one shell line in `cwd`, with a home and a temp folder in `dir`.
fn sh(dir: &Path, cwd: &Path, line: &str) -> String {
    let path = format!("{}:/usr/bin:/bin", dir.join("bin").display());
    let store = format!("file://{}/store", dir.display());
    let out = Command::new("/bin/sh")
        .args(["-c", line])
        .current_dir(cwd)
        .env("HOME", dir.join("home"))
        .env("TMPDIR", dir.join("tmp"))
        .env("PATH", path)
        .env("CODEUMBRELLA_STORE", store)
        .output()
        .expect("start a shell");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    text
}

#[test]
fn install_script_makes_its_temp_folder_in_tmpdir() {
    let dir = std::env::temp_dir().join(format!("cu-install-temp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let (repo, bin, tmp) = (dir.join("repo"), dir.join("bin"), dir.join("tmp"));
    for folder in [&repo, &bin, &tmp, &dir.join("home")] {
        fs::create_dir_all(folder).expect("make a folder");
    }
    for tool in ["gh", "claude"] {
        fs::write(bin.join(tool), "#!/bin/sh\nexit 0\n").expect("write a tool");
        let mode = fs::Permissions::from_mode(0o755);
        fs::set_permissions(bin.join(tool), mode).expect("set the mode");
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh");
    fs::copy(script, repo.join("install.sh")).expect("copy install.sh");
    let commit =
        "git init -q && git add . && git -c user.name=t -c user.email=t@t.t commit -q -m t";
    sh(&dir, &repo, commit);

    // The store has no file, so the script stops after it makes the folder.
    let text = sh(&dir, &repo, "./install.sh");

    assert!(text.contains("the store has no file"), "{text}");
    let made = fs::read_dir(&tmp)
        .expect("read the temp folder")
        .flatten()
        .any(|entry| entry.path().is_dir());
    assert!(made, "no new folder in TMPDIR: {text}");
}
