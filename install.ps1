#Requires -Version 5.1
<#
.SYNOPSIS
    Install the active-collab CLI for Windows.
.PARAMETER Version
    Release tag to install (e.g. "v0.1.0"). Defaults to the latest release.
.EXAMPLE
    irm https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.ps1 | iex
.EXAMPLE
    .\install.ps1 -Version v0.1.0
#>
param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$Repo   = "ejklock/active-collab-cli"
$Asset  = "active-collab-windows-x86_64.exe"
$BinDir = Join-Path $env:LOCALAPPDATA "Programs\active-collab"
$Dest   = Join-Path $BinDir "active-collab.exe"

if ($Version -eq "") {
    $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
    try {
        $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "active-collab-installer" }
        $Version = $release.tag_name
    } catch {
        Write-Error "Could not determine the latest release tag: $_"
        exit 1
    }
}

$DownloadUrl = "https://github.com/$Repo/releases/download/$Version/$Asset"

Write-Host "Downloading $Asset ($Version) ..."
if (-not (Test-Path $BinDir)) {
    New-Item -ItemType Directory -Path $BinDir | Out-Null
}

try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $Dest -UseBasicParsing
} catch {
    Write-Error "Download failed: $_"
    exit 1
}

$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($userPath -notlike "*$BinDir*") {
    [Environment]::SetEnvironmentVariable(
        "PATH",
        "$BinDir;$userPath",
        "User"
    )
    Write-Host "Added $BinDir to your user PATH."
    Write-Host "Restart your terminal (or open a new one) for it to take effect."
}

Write-Host "Installed to $Dest"
& $Dest --help
