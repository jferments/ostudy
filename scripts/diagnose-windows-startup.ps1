# Collects why a Windows build of ostudy stops at startup, in one report.
#
# Run from the repository root:
#   powershell -ExecutionPolicy Bypass -File .\scripts\diagnose-windows-startup.ps1
# It prints the report and saves it to %TEMP%\ostudy-startup-report.txt.
# It closes running ostudy processes and moves the library aside (renamed,
# never deleted) so the app starts on an empty library. -KeepLibrary starts
# on the existing library instead.

param(
    [switch]$KeepLibrary,
    [int]$WaitSeconds = 25
)

$ErrorActionPreference = "Continue"
$RootDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Report = Join-Path $env:TEMP "ostudy-startup-report.txt"
$Lines = New-Object System.Collections.Generic.List[string]

function Say([string]$Text) {
    $Lines.Add($Text)
    Write-Host $Text
}

function Section([string]$Title) {
    Say ""
    Say "== $Title"
}

function Sha256([byte[]]$Bytes) {
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($hasher.ComputeHash($Bytes)) -replace "-", "").ToLowerInvariant() }
    finally { $hasher.Dispose() }
}

Set-Location $RootDir
Say "ostudy Windows startup report, $(Get-Date -Format o)"

Section "Checkout"
Say ("HEAD: " + (git rev-parse --short HEAD 2>&1))
Say ("core.autocrlf: " + (git config core.autocrlf 2>&1))
Say ("Changed tracked files: " + @(git status --porcelain --untracked-files=no 2>$null).Count)
$crlf = @(git ls-files --eol 2>$null | Where-Object { $_ -match "w/crlf" })
Say ("Tracked files checked out with CRLF: " + $crlf.Count)
$crlf | Select-Object -First 10 | ForEach-Object { Say "  $_" }

Section "Migrations against the checksums the app expects"
$databaseSource = Get-Content -Raw (Join-Path $RootDir "src-tauri\src\database.rs")
$pattern = 'checksum:\s*"([0-9a-f]{64})",\s*sql:\s*include_str!\("\.\./migrations/([^"]+)"\)'
$expected = [regex]::Matches($databaseSource, $pattern)
$bad = 0
foreach ($entry in $expected) {
    $name = $entry.Groups[2].Value
    $path = Join-Path $RootDir "src-tauri\migrations\$name"
    $bytes = [IO.File]::ReadAllBytes($path)
    $actual = Sha256 $bytes
    $hasCr = [Array]::IndexOf($bytes, [byte]13) -ge 0
    if ($actual -ne $entry.Groups[1].Value) {
        $bad++
        Say "  MISMATCH $name (CR bytes: $hasCr)"
    }
}
Say "  $($expected.Count) migrations checked, $bad mismatched"

Section "Built and installed app"
$migration = Join-Path $RootDir "src-tauri\migrations\0002_canonical_documents.sql"
$lfText = ([IO.File]::ReadAllText($migration)) -replace "`r`n", "`n"
$crlfText = $lfText -replace "`n", "`r`n"
# Compare as Latin-1 text: one char per byte, and String.IndexOf is fast.
$latin1 = [Text.Encoding]::GetEncoding(28591)
$lfProbe = $latin1.GetString([Text.Encoding]::UTF8.GetBytes($lfText.Substring(0, [Math]::Min(400, $lfText.Length))))
$crlfProbe = $latin1.GetString([Text.Encoding]::UTF8.GetBytes($crlfText.Substring(0, [Math]::Min(400, $crlfText.Length))))
$candidates = @(
    (Join-Path $RootDir "src-tauri\target\release\ostudy.exe"),
    (Join-Path $env:LOCALAPPDATA "ostudy\ostudy.exe"),
    (Join-Path $env:ProgramFiles "ostudy\ostudy.exe")
)
$built = $null
foreach ($exe in $candidates) {
    if (-not (Test-Path $exe)) { Say "  $exe : not present"; continue }
    if ($null -eq $built) { $built = $exe }
    $item = Get-Item $exe
    $bytes = [IO.File]::ReadAllBytes($exe)
    $text = $latin1.GetString($bytes)
    $lfFound = $text.IndexOf($lfProbe, [StringComparison]::Ordinal) -ge 0
    $crlfFound = $text.IndexOf($crlfProbe, [StringComparison]::Ordinal) -ge 0
    Say "  $exe"
    Say "    modified $($item.LastWriteTime.ToString('s')), sha256 $((Sha256 $bytes).Substring(0, 16))"
    Say "    migration 2 compiled in with LF: $lfFound, with CRLF: $crlfFound"
}
Say ("  Migration 2 file modified: " + (Get-Item $migration).LastWriteTime.ToString("s"))

Section "Library"
$appData = Join-Path $env:APPDATA "io.github.jferments.ostudy"
Get-Process ostudy -ErrorAction SilentlyContinue | ForEach-Object {
    Say "  Closing running ostudy process $($_.Id)"
    $_ | Stop-Process -Force
}
Start-Sleep -Seconds 1
if (Test-Path $appData) {
    Get-ChildItem $appData -Force | ForEach-Object { Say "  $($_.Name)" }
    if (-not $KeepLibrary) {
        $aside = "$appData.before-$(Get-Date -Format yyyyMMdd-HHmmss)"
        Move-Item $appData $aside
        Say "  Moved the library aside to $aside"
    }
} else {
    Say "  No library at $appData"
}

Section "Starting the app"
if ($null -eq $built) {
    Say "  No ostudy.exe found; build it first."
} else {
    $stderr = Join-Path $env:TEMP "ostudy-startup-stderr.txt"
    $stdout = Join-Path $env:TEMP "ostudy-startup-stdout.txt"
    Remove-Item $stderr, $stdout -ErrorAction SilentlyContinue
    $env:OSTUDY_LOG = "debug"
    Say "  Running $built for up to $WaitSeconds s (click OK on any error dialog)"
    $process = Start-Process $built -PassThru -RedirectStandardError $stderr -RedirectStandardOutput $stdout
    if (-not $process.WaitForExit($WaitSeconds * 1000)) {
        Say "  Still running after $WaitSeconds s: it started. Closing it."
        $process | Stop-Process -Force
    } else {
        Say "  Exited with code $($process.ExitCode)"
    }
    Remove-Item Env:\OSTUDY_LOG -ErrorAction SilentlyContinue
    Say "  -- stderr"
    if (Test-Path $stderr) { Get-Content $stderr | Select-Object -Last 40 | ForEach-Object { Say "  $_" } }
}

Section "Diagnostics log"
$log = Join-Path $appData "diagnostics\ostudy.jsonl"
if (Test-Path $log) {
    Get-Content $log | Select-Object -Last 40 | ForEach-Object { Say "  $_" }
} else {
    Say "  No log at $log"
}

$Lines | Set-Content -Encoding UTF8 $Report
Say ""
Say "Report saved to $Report"
try { $Lines -join "`r`n" | Set-Clipboard; Say "Report copied to the clipboard." } catch { }
