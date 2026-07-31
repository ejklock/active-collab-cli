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
$Shim   = Join-Path $BinDir "ac.cmd"

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

# Windows symlinks require elevation or Developer Mode, so the short `ac` command
# the docs and the agent skill use is provided as a .cmd forwarder, which both
# cmd.exe and PowerShell resolve from PATH.
$shimBody = "@echo off`r`n`"%~dp0active-collab.exe`" %*`r`n"
Set-Content -Path $Shim -Value $shimBody -Encoding ASCII -NoNewline

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
Write-Host "Short command available: ac (via $Shim)"

$onPath = Get-Command ac -ErrorAction SilentlyContinue
if ($onPath -and $onPath.Source -ne $Shim) {
    Write-Warning "'ac' still resolves to $($onPath.Source), which comes earlier on your PATH."
    Write-Warning "Put $BinDir ahead of it, or run 'active-collab'."
}
& $Dest --help
