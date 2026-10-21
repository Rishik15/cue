# Purpose: Keep the cached build of sherpa-onnx-sys honest. Its build script downloads the native speech library into
# target/sherpa-onnx-prebuilt once, and Cargo caches only the script's result (a link path). A restored cache can hold the
# result but not the library, or an empty folder where it was: the script then trusts the folder (it only checks that
# `lib` exists) and the link fails with "could not find native static library". Unless the library file is really there,
# remove the folder and the cached build of the crate so the script downloads the library again. Run after the Rust cache is
# restored, before building.
$target = Join-Path $PSScriptRoot "..\src-tauri\target"
$library = Join-Path $target "sherpa-onnx-prebuilt"
$found = Test-Path $library -PathType Container
if ($found) {
    $found = [bool](Get-ChildItem $library -Recurse -File -Filter "sherpa-onnx-c-api.lib" -ErrorAction SilentlyContinue | Select-Object -First 1)
}
if ($found) {
    Write-Host "sherpa-onnx library present: the cached build is kept."
    exit 0
}
Write-Host "sherpa-onnx library missing: dropping the folder and the cached build of sherpa-onnx-sys so it downloads again."
Remove-Item $library -Recurse -Force -ErrorAction SilentlyContinue
Get-ChildItem $target -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    foreach ($part in "build", ".fingerprint") {
        Get-ChildItem (Join-Path $_.FullName $part) -Directory -Filter "sherpa-onnx-sys-*" -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force
    }
}
