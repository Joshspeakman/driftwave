# Builds Shortwave for Windows into windows\out\<Flavor> (framework-dependent, x64).
# Usage: windows\scripts\build.ps1 [-Flavor Nightly|Release] [-Configuration Release|Debug]
param(
    [ValidateSet('Nightly', 'Release')][string]$Flavor = 'Nightly',
    [ValidateSet('Release', 'Debug')][string]$Configuration = 'Release'
)
. "$PSScriptRoot\common.ps1"

$version = '1.0'
$toml = Join-Path $RepoRoot 'VERSION.toml'
if (Test-Path $toml) {
    $m = Select-String -Path $toml -Pattern '^version\s*=\s*"(.*)"' | Select-Object -First 1
    if ($m) { $version = $m.Matches[0].Groups[1].Value }
}
Push-Location $RepoRoot
try {
    $buildNo = (git rev-list --count HEAD 2>$null); if (-not $buildNo) { $buildNo = '1' }
    $commit = (git rev-parse --short HEAD 2>$null); if (-not $commit) { $commit = 'dev' }
    $branch = (git rev-parse --abbrev-ref HEAD 2>$null); if (-not $branch) { $branch = '' }
}
finally { Pop-Location }

$out = Get-BuildDir $Flavor
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
$project = Join-Path $WindowsRoot 'src\Shortwave\Shortwave.csproj'
& dotnet publish $project -c $Configuration -r win-x64 --self-contained false -o $out `
    "-p:Flavor=$Flavor" "-p:Version=$version.$buildNo" "-p:InformationalVersion=$version.$buildNo+$commit" `
    "-p:ShortwaveCommit=$commit" "-p:ShortwaveBranch=$branch" -nologo -v:minimal
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed ($LASTEXITCODE)" }
$exe = Join-Path $out 'Shortwave.exe'
if (-not (Test-Path $exe)) { throw "Build output missing: $exe" }
Write-Output $exe
