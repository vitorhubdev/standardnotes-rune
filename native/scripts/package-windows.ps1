$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$versionMatch = Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"'
$version = $versionMatch.Matches[0].Groups[1].Value
if (!$version) { throw 'Workspace version missing' }
$name = "standardnotes-rune-$version-windows-x86_64"
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$binary = Join-Path $target 'release/standardnotes-rune.exe'
if (!(Test-Path $binary)) { throw "Executable missing: $binary" }
New-Item -ItemType Directory -Force "dist/$name" | Out-Null
Copy-Item $binary "dist/$name/standardnotes-rune.exe"
Copy-Item '../LICENSE','README.md','MIGRATION-COVERAGE.md' "dist/$name/"
Copy-Item -Recurse 'third-party' "dist/$name/third-party"
$rustSysroot = (& rustc --print sysroot).Trim()
New-Item -ItemType Directory -Force "dist/$name/third-party/rust-stdlib" | Out-Null
Copy-Item (Join-Path $rustSysroot 'share/doc/rust/COPYRIGHT-library.html') "dist/$name/third-party/rust-stdlib/"
Copy-Item -Recurse (Join-Path $rustSysroot 'share/doc/rust/licenses') "dist/$name/third-party/rust-stdlib/licenses"
& rustc -Vv | Set-Content -Encoding utf8 "dist/$name/BUILD-INFO.txt"
& git rev-parse HEAD | Add-Content -Encoding utf8 "dist/$name/BUILD-INFO.txt"
Copy-Item $binary "dist/$name.exe"
Compress-Archive -Path "dist/$name/*" -DestinationPath "dist/$name.zip" -Force
$checksums = @("dist/$name.exe", "dist/$name.zip") | ForEach-Object {
  $hash = Get-FileHash -Algorithm SHA256 -Path $_
  "$($hash.Hash.ToLowerInvariant())  $([System.IO.Path]::GetFileName($_))"
}
$checksums | Set-Content -Encoding ascii 'dist/SHA256SUMS-windows.txt'
Remove-Item -Recurse -Force "dist/$name"
