// Compresses payload/pulse.exe into OUT_DIR so the setup exe carries the app inside itself.
use std::{env, fs, io::Write, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=payload/pulse.exe");
    println!("cargo:rerun-if-changed=payload/LICENSE.txt");

    let payload = Path::new("payload/pulse.exe");
    if !payload.exists() {
        if env::var("PROFILE").map(|p| p == "release").unwrap_or(false) {
            panic!(
                "payload/pulse.exe is missing. Build the installer with scripts/build-installer.ps1 \
                 (it builds Pulse first and copies the exe into installer/src-tauri/payload)."
            );
        }
        // Debug / `tauri dev`: a placeholder lets you work on the setup UI without building Pulse.
        fs::create_dir_all("payload").unwrap();
        fs::write(payload, b"PULSE DEV PLACEHOLDER - not a real executable").unwrap();
        println!("cargo:warning=payload/pulse.exe was missing; wrote a dev placeholder (do not ship this build)");
    }

    let raw = fs::read(payload).expect("cannot read payload/pulse.exe");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(9));
    encoder.write_all(&raw).unwrap();
    let gz = encoder.finish().unwrap();

    let out = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out).join("pulse.exe.gz"), gz).unwrap();
    fs::write(
        Path::new(&out).join("payload_meta.rs"),
        format!("pub const PAYLOAD_SIZE: u64 = {};\n", raw.len()),
    )
    .unwrap();

    tauri_build::build();
}
