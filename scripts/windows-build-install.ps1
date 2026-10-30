# Builds the Windows installer from the checked-out commit, installs it, and
# runs the startup diagnostic against the installed app.
#
# Run from the repository root after checking out the commit to test:
#   powershell -ExecutionPolicy Bypass -File .\scripts\windows-build-install.ps1 -ExpectedCommit <sha>
# It stops at the first failing step and says which one. Tracked files are
# rewritten only when they were checked out with CRLF line endings, which
# discards uncommitted changes to tracked files in this checkout.

param(
    [string]$ExpectedCommit = "",
    [switch]$KeepLibrary
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $RootDir

function Step([string]$Title) {
    Write-Host ""
    Write-Host "== $Title" -ForegroundColor Cyan
}

function Run([string]$Title, [scriptblock]$Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Step failed: $Title (exit code $LASTEXITCODE)"
    }
}

Step "Commit"
$head = (git rev-parse HEAD).Trim()
Write-Host "HEAD $head"
if ($ExpectedCommit -and -not $head.StartsWith($ExpectedCommit)) {
    throw "WRONG VERSION: HEAD is $head, expected $ExpectedCommit. Run git fetch origin and check out the branch again."
}

Step "Line endings"
$crlf = @(git ls-files --eol | Where-Object { $_ -match "w/crlf" })
if ($crlf.Count -gt 0) {
    Write-Host "$($crlf.Count) tracked files have CRLF line endings; rewriting them with LF."
    Run "remove index" { git rm -r -q --cached . }
    Run "rewrite files" { git reset -q --hard HEAD }
    $crlf = @(git ls-files --eol | Where-Object { $_ -match "w/crlf" })
    if ($crlf.Count -gt 0) { throw "Still $($crlf.Count) CRLF files after rewriting, e.g. $($crlf[0])" }
}
Write-Host "All tracked files use LF."

Step "PDFium"
Run "install PDFium" { powershell -ExecutionPolicy Bypass -File .\scripts\install-native-deps.ps1 -PdfiumOnly }

Step "JavaScript packages"
Run "pnpm install" { pnpm install --frozen-lockfile }

Step "Build the installer (20-40 minutes the first time)"
Get-Process ostudy -ErrorAction SilentlyContinue | Stop-Process -Force
Run "tauri build" { pnpm tauri build --bundles nsis '--' --locked }
$installer = Get-ChildItem (Join-Path $RootDir "src-tauri\target\release\bundle\nsis") -Filter "*-setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($null -eq $installer) { throw "The build made no installer." }
Write-Host "Installer $($installer.FullName)"

Step "Install"
Get-Process ostudy -ErrorAction SilentlyContinue | Stop-Process -Force
$installDir = Join-Path $env:LOCALAPPDATA "ostudy"
# Earlier installers left pdfium.dll as a file named native\pdfium, which
# blocks the folder of the same name.
$stale = Join-Path $installDir "native\pdfium"
if (Test-Path $stale -PathType Leaf) { Remove-Item -Force $stale }
$setup = Start-Process $installer.FullName -ArgumentList "/S" -PassThru -Wait
if ($setup.ExitCode -ne 0) { throw "The installer exited with code $($setup.ExitCode)." }
$exe = Join-Path $installDir "ostudy.exe"
$dll = Join-Path $installDir "native\pdfium\pdfium.dll"
if (-not (Test-Path $exe)) { throw "No installed app at $exe." }
if (-not (Test-Path $dll -PathType Leaf)) { throw "The installer did not put PDFium at $dll." }
Write-Host "Installed $exe with $dll"

Step "Start the installed app"
$diagnose = @{ Exe = $exe }
if ($KeepLibrary) { $diagnose.KeepLibrary = $true }
& (Join-Path $RootDir "scripts\diagnose-windows-startup.ps1") @diagnose
