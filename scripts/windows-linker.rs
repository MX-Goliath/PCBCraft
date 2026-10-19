//! Adapter for this workspace's optional portable Windows GNU toolchain.
//! GCC's bundled binutils linker mislinks the mixed raw-dylib imports used by
//! the GUI dependencies. Use Rust's LLVM linker with the x64 MinGW DLL entry.
use std::{
    env,
    process::{Command, exit},
};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let root = env::current_exe()
        .expect("linker path")
        .parent()
        .expect("tools directory")
        .to_path_buf();
    let lld = root.join("rustup/toolchains/1.90.0-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/rust-lld.exe");
    let mut command = Command::new(lld);
    command.args(&args);
    let shared = args.iter().any(|arg| {
        if arg == "-shared" {
            return true;
        }
        let text = arg.to_string_lossy();
        let Some(path) = text.strip_prefix('@') else {
            return false;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let content = if bytes.starts_with(&[0xff, 0xfe]) {
            String::from_utf16_lossy(
                &bytes[2..]
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect::<Vec<_>>(),
            )
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        content.contains("-shared")
    });
    if shared {
        command.arg("--entry=DllMainCRTStartup");
    }
    exit(
        command
            .status()
            .expect("Could not launch rust-lld")
            .code()
            .unwrap_or(1),
    );
}
