# Purpose: Keep the cached build of sherpa-onnx-sys honest. Its build script downloads the native speech library into
# target/sherpa-onnx-prebuilt once, and Cargo caches only the script's result (a link path). A restored cache that has the result
# but not the folder fails with "could not find native static library". When the folder is missing or empty, drop the cached
# build of the crate so the script runs again and downloads the library. Run after the Rust cache is restored, before building.
$target = Join-Path $PSScriptRoot "..\src-tauri\target"
$library = Join-Path $target "sherpa-onnx-prebuilt"
if ((Test-Path $library) -and (Get-ChildItem $library -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 1)) {
    Write-Host "sherpa-onnx library present: the cached build is kept."
    exit 0
}
Write-Host "sherpa-onnx library missing: dropping the cached build of sherpa-onnx-sys so it downloads again."
Get-ChildItem $target -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    foreach ($part in "build", ".fingerprint") {
        Get-ChildItem (Join-Path $_.FullName $part) -Directory -Filter "sherpa-onnx-sys-*" -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force
    }
}
