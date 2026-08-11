param(
    [switch]$PdfiumOnly,
    [switch]$WithTesseract,
    [string]$Target = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$ManifestPath = Join-Path $ScriptDir "native-deps-manifest.json"
$PdfiumDest = Join-Path $RootDir "src-tauri\resources\native\pdfium"
$InstallTesseract = -not $PdfiumOnly -or $WithTesseract

if (-not (Test-Path $ManifestPath)) {
    throw "Native dependency manifest is missing: $ManifestPath"
}

$manifest = Get-Content -Raw -Path $ManifestPath | ConvertFrom-Json

if ([string]::IsNullOrWhiteSpace($Target)) {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
    $Target = switch ($arch) {
        "x64" { "windows-x64" }
        "arm64" { "windows-arm64" }
        "x86" { "windows-x86" }
        default { throw "Unsupported Windows architecture '$arch'. Pass -Target using a manifest target." }
    }
}

$artifact = $manifest.pdfium.artifacts.PSObject.Properties[$Target].Value
if ($null -eq $artifact) {
    throw "No pinned PDFium artifact is defined for target '$Target'."
}

$build = [string]$manifest.pdfium.apiBuild
$baseUrl = [string]$manifest.pdfium.binarySource.releaseBaseUrl
$archive = [string]$artifact.archive
$expectedSha = ([string]$artifact.sha256).ToLowerInvariant()
$libraryPath = ([string]$artifact.library).Replace('/', [IO.Path]::DirectorySeparatorChar)
$libraryName = Split-Path -Leaf $libraryPath
$installedLibrary = Join-Path $PdfiumDest $libraryName
$provenance = Join-Path $PdfiumDest "PROVENANCE.env"

if (-not [string]::IsNullOrWhiteSpace($env:OSTUDY_PDFIUM_PATH)) {
    $explicit = $env:OSTUDY_PDFIUM_PATH
    if (Test-Path $explicit -PathType Container) {
        $explicit = Join-Path $explicit $libraryName
    }
    if (-not (Test-Path $explicit -PathType Leaf)) {
        throw "OSTUDY_PDFIUM_PATH does not resolve to a PDFium library: $explicit"
    }
    Write-Host "Using operator-managed PDFium: $explicit"
} else {
    $reuse = $false
    if ((Test-Path $installedLibrary -PathType Leaf) -and (Test-Path $provenance -PathType Leaf)) {
        $metadata = @{}
        foreach ($line in Get-Content $provenance) {
            $parts = $line -split '=', 2
            if ($parts.Count -eq 2) { $metadata[$parts[0]] = $parts[1] }
        }
        $installedSha = (Get-FileHash -Algorithm SHA256 -Path $installedLibrary).Hash.ToLowerInvariant()
        if ($metadata.target -eq $Target -and
            $metadata.archive_sha256 -eq $expectedSha -and
            $metadata.library_sha256 -eq $installedSha) {
            $reuse = $true
            Write-Host "OK   PDFium $build for $Target is already staged and verified."
        }
    }

    if (-not $reuse) {
        $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("ostudy-pdfium-" + [Guid]::NewGuid().ToString("N"))
        $extractDir = Join-Path $tempRoot "extract"
        $archivePath = Join-Path $tempRoot $archive
        New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
        try {
            $url = "$baseUrl/$archive"
            Write-Host "Downloading pinned PDFium $build for $Target..."
            Invoke-WebRequest -Uri $url -OutFile $archivePath
            $actualSha = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLowerInvariant()
            if ($actualSha -ne $expectedSha) {
                throw "PDFium archive SHA-256 mismatch. Expected $expectedSha, got $actualSha."
            }

            & tar -xzf $archivePath -C $extractDir
            if ($LASTEXITCODE -ne 0) { throw "tar failed while extracting PDFium." }
            $extractedLibrary = Join-Path $extractDir $libraryPath
            if (-not (Test-Path $extractedLibrary -PathType Leaf)) {
                throw "Verified archive does not contain expected library path '$libraryPath'."
            }

            if (Test-Path $PdfiumDest) { Remove-Item -Recurse -Force $PdfiumDest }
            New-Item -ItemType Directory -Force -Path $PdfiumDest | Out-Null
            Copy-Item $extractedLibrary $installedLibrary
            $license = Join-Path $extractDir "LICENSE"
            if (Test-Path $license) { Copy-Item $license (Join-Path $PdfiumDest "PDFIUM-LICENSE.txt") }
            $librarySha = (Get-FileHash -Algorithm SHA256 -Path $installedLibrary).Hash.ToLowerInvariant()
            @(
                "pdfium_build=$build"
                "target=$Target"
                "archive=$archive"
                "archive_sha256=$expectedSha"
                "library=$libraryName"
                "library_sha256=$librarySha"
                "source=$url"
            ) | Set-Content -Encoding UTF8 -Path $provenance
            Write-Host "OK   PDFium $build staged at $installedLibrary"
        } finally {
            if (Test-Path $tempRoot) { Remove-Item -Recurse -Force $tempRoot }
        }
    }
}

if ($InstallTesseract) {
    $tesseract = Get-Command tesseract -ErrorAction SilentlyContinue
    if ($null -eq $tesseract) {
        $common = "C:\Program Files\Tesseract-OCR\tesseract.exe"
        if (Test-Path $common) {
            $tesseractPath = $common
        } else {
            if ($null -eq (Get-Command winget -ErrorAction SilentlyContinue)) {
                throw "Tesseract is missing and winget is unavailable. Install Tesseract 5 using a distribution documented by the Tesseract project, then retry."
            }
            Write-Host "Installing Tesseract through the Tesseract-documented UB Mannheim Windows distribution (winget)..."
            & winget install --exact --id UB-Mannheim.TesseractOCR --accept-package-agreements --accept-source-agreements
            if ($LASTEXITCODE -ne 0) { throw "winget failed to install Tesseract OCR." }
            if (Test-Path $common) {
                $tesseractPath = $common
            } else {
                $tesseract = Get-Command tesseract -ErrorAction SilentlyContinue
                if ($null -eq $tesseract) { throw "Tesseract installed but its executable could not be located." }
                $tesseractPath = $tesseract.Source
            }
        }
    } else {
        $tesseractPath = $tesseract.Source
    }

    $languages = & $tesseractPath --list-langs 2>$null
    if ($languages -notcontains "eng") {
        throw "Tesseract is available but English trained data ('eng') is missing."
    }
    Write-Host "OK   Tesseract with English trained data is available at $tesseractPath"
    Write-Host "For ostudy sessions where it is not on PATH, set OSTUDY_TESSERACT_PATH='$tesseractPath'."
}
