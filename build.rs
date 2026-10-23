//! Embed the original P icon without adding native resource build dependencies.
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn executable(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(name);
    if path.is_file() {
        return Some(path);
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

fn msvc_resource_compiler() -> Option<PathBuf> {
    if let Some(compiler) = env::var_os("RC").and_then(|p| executable(&p.to_string_lossy())) {
        return Some(compiler);
    }
    if let Some(compiler) = executable("rc.exe") {
        return Some(compiler);
    }
    let sdk = env::var_os("WindowsSdkDir")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Windows Kits/10"))
        })?;
    let bin = sdk.join("bin");
    let mut versions = std::fs::read_dir(&bin)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect::<Vec<_>>();
    versions.sort();
    versions.reverse();
    versions
        .into_iter()
        .map(|p| p.join("x64/rc.exe"))
        .chain(Some(bin.join("x64/rc.exe")))
        .find(|p| p.is_file())
}

fn run(mut command: Command) {
    let output = command
        .output()
        .expect("Cannot start Windows resource compiler; install Windows SDK or MinGW windres.");
    assert!(
        output.status.success(),
        "Windows icon resource compilation failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn main() {
    // This resource template has no C macros/includes. Let windres read it
    // through this binary, so Rust GNU's link-only GCC needs no C preprocessor.
    if env::args().any(|arg| arg == "--fpdf-resource-pass-through") {
        let input = env::args_os().last().expect("Resource input path");
        use std::io::Write;
        std::io::stdout()
            .write_all(&std::fs::read(input).expect("Read resource input"))
            .unwrap();
        return;
    }
    println!("cargo:rerun-if-changed=assets/fpdf.ico");
    println!("cargo:rerun-if-changed=assets/fpdf.rc.in");
    for name in ["RC", "WINDRES", "WindowsSdkDir", "ProgramFiles(x86)"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        // Keep bundled X11 runtime libraries discoverable through the launcher symlink.
        println!("cargo:rustc-link-arg-bin=fpdf=-Wl,--disable-new-dtags,-rpath,$ORIGIN");
        return;
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = version
        .split('.')
        .take(3)
        .map(|part| part.parse::<u16>().expect("Numeric application version"))
        .collect::<Vec<_>>();
    while parts.len() < 4 {
        parts.push(0);
    }
    let icon = root.join("assets/fpdf.ico");
    let input = std::fs::read_to_string(root.join("assets/fpdf.rc.in"))
        .unwrap()
        .trim_start_matches('\u{feff}')
        .replace("@ICON@", &icon.to_string_lossy().replace('\\', "/"))
        .replace(
            "@VERSION_COMMAS@",
            &parts
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
        .replace("@VERSION@", &version);
    let rc = out.join("fpdf.rc");
    std::fs::write(&rc, input).unwrap();
    let resource = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let resource = out.join("fpdf.res");
        let compiler=msvc_resource_compiler().expect("Windows SDK rc.exe not found. Install Visual Studio Desktop development with C++ and Windows SDK.");
        let mut command = Command::new(compiler);
        command.arg("/nologo").arg("/fo").arg(&resource).arg(&rc);
        run(command);
        resource
    } else {
        let target = env::var("TARGET").unwrap();
        let prefix = target.replace("pc-windows-gnu", "w64-mingw32");
        let compiler = env::var("WINDRES")
            .ok()
            .and_then(|p| executable(&p))
            .or_else(|| executable(&format!("{prefix}-windres.exe")))
            .or_else(|| executable("windres.exe"))
            .expect("MinGW windres.exe not found.");
        let resource = out.join("fpdf-icon.o");
        let mut command = Command::new(compiler);
        command
            .arg("--input")
            .arg(&rc)
            .arg("--output")
            .arg(&resource)
            .arg("--output-format=coff");
        command
            .arg("--preprocessor")
            .arg(format!("\"{}\"", env::current_exe().unwrap().display()))
            .arg("--preprocessor-arg=--fpdf-resource-pass-through");
        if target.starts_with("x86_64") {
            command.arg("--target=pe-x86-64");
        }
        if target.starts_with("i686") {
            command.arg("--target=pe-i386");
        }
        run(command);
        resource
    };
    assert!(Path::new(&resource).is_file());
    println!("cargo:rustc-link-arg-bin=fpdf={}", resource.display());
}
