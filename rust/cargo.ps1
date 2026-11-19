# Runs cargo with this PC's Rust toolchain (D:\DevCaches), without changing any user environment variables.
# Usage: rust\cargo.ps1 test   |   rust\cargo.ps1 build --release
$env:RUSTUP_HOME = 'D:\DevCaches\rustup'
$env:CARGO_HOME = 'D:\DevCaches\cargo'
$env:PATH = "D:\DevCaches\cargo\bin;$env:PATH"
Push-Location $PSScriptRoot
try { & cargo @args; exit $LASTEXITCODE } finally { Pop-Location }
