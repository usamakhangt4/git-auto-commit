$ErrorActionPreference = "Stop"

$repo = "usamakhangt4/git-auto-commit"
$installDir = if ($env:GIT_AUTO_COMMIT_INSTALL_DIR) {
    $env:GIT_AUTO_COMMIT_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "git-auto-commit\bin"
}

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Only 64-bit Windows is currently supported."
}

$url = "https://github.com/$repo/releases/latest/download/git-auto-commit-x86_64-pc-windows-msvc.zip"
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("git-auto-commit-" + [guid]::NewGuid())

try {
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
    $archive = Join-Path $tempDir "archive.zip"
    Write-Host "Downloading git-auto-commit for Windows x64..."
    Invoke-WebRequest -Uri $url -OutFile $archive
    Expand-Archive -Path $archive -DestinationPath $tempDir -Force
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item (Join-Path $tempDir "git-auto-commit.exe") $installDir -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $paths = @($userPath -split ";" | Where-Object { $_ })
    if ($paths -notcontains $installDir) {
        $newPath = (@($paths) + $installDir) -join ";"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "Added $installDir to your user PATH. Open a new terminal to use it."
    }

    Write-Host "Installed git-auto-commit to $installDir\git-auto-commit.exe"
    Write-Host "Run: git-auto-commit --help"
} finally {
    if (Test-Path $tempDir) { Remove-Item $tempDir -Recurse -Force }
}

