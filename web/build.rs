use std::{env, fs, path::Path, process::Command};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    build_info(&root);
    let out = env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR");
    println!("cargo:rerun-if-env-changed=XETAL_BIN");
    for path in ["assemble", "assemble.xtl", "F18.xtl"] {
        println!("cargo:rerun-if-changed=../{path}");
    }
    for demo in [
        "double",
        "sum",
        "send",
        "receive",
        "relay-double",
        "receive-relay",
    ] {
        let source = format!("examples/{demo}.f18");
        println!("cargo:rerun-if-changed=../{source}");
        let output = Command::new(root.join("assemble"))
            .arg(root.join(source))
            .output()
            .expect("launch X_eTaL assembler");
        assert!(
            output.status.success(),
            "assemble {demo}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::write(Path::new(&out).join(format!("{demo}.img")), output.stdout)
            .expect("write assembled demo");
    }
}

fn build_info(root: &Path) {
    for path in ["src", "index.html", "studio.css", "Cargo.lock"] {
        println!("cargo:rerun-if-changed={path}");
    }

    let git = |args: &[&str]| command_text(Command::new("git").current_dir(root).args(args));
    let head_ref = git(&["symbolic-ref", "-q", "HEAD"]);
    for git_file in ["HEAD", "packed-refs"]
        .into_iter()
        .chain(head_ref.as_deref())
    {
        if let Some(path) = git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            git_file,
        ]) && Path::new(&path).is_file()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let host = command_text(&mut Command::new("hostname"));
    let revision = git(&["rev-parse", "--short", "HEAD"]);
    let time = command_text(Command::new("date").args(["-u", "+%Y-%m-%d %H:%M:%S UTC"]));
    println!(
        "cargo:rustc-env=GA144_BUILD=build({} {} {})",
        host.as_deref().unwrap_or("unknown-host"),
        revision.as_deref().unwrap_or("unknown-revision"),
        time.as_deref().unwrap_or("unknown-time"),
    );
}

fn command_text(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
