//! On Windows the program's own file carries the logo, so that Explorer and a pinned
//! taskbar button show it. Everywhere else there is nothing to do: the window gets its
//! icon when it opens.

use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let here = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let icon = here.join("../../packaging/windows/suit.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let out = PathBuf::from(env::var("OUT_DIR").expect("set by cargo"));

    // The resource compiler wants its paths without spaces and with plain slashes, so the
    // icon is put next to the script that names it.
    let copied = fs::copy(&icon, out.join("suit.ico")).is_ok();
    let written = fs::write(out.join("suit.rc"), "1 ICON \"suit.ico\"\n").is_ok();
    let compilers = ["x86_64-w64-mingw32-windres", "windres"];
    let compiled = copied
        && written
        && compilers.iter().any(|windres| {
            let run = Command::new(windres)
                .current_dir(&out)
                .args(["suit.rc", "-O", "coff", "-o", "suit-icon.o"])
                .status();
            run.is_ok_and(|status| status.success())
        });
    match compiled {
        true => println!(
            "cargo:rustc-link-arg-bins={}",
            out.join("suit-icon.o").display()
        ),
        false => println!("cargo:warning=suit.exe carries no icon: no resource compiler found"),
    }
}
