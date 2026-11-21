# Produces a single-file portable build. Pulse stores its data in "pulse-data"
# next to the exe whenever the exe's file name ends in "-portable".
$ErrorActionPreference = "Stop"
$out = "dist-portable"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "src-tauri/target/release/pulse.exe" "$out/Pulse-portable.exe" -Force
Write-Host "Portable build: $out/Pulse-portable.exe"
