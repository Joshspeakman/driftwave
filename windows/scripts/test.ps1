# Core unit tests (the 19 translated from the Swift suite, plus the Windows-only ones).
# Usage: windows\scripts\test.ps1
. "$PSScriptRoot\common.ps1"
& dotnet test (Join-Path $WindowsRoot 'tests\ShortwaveCore.Tests\ShortwaveCore.Tests.csproj') -nologo -v:quiet
exit $LASTEXITCODE
