param(
    [string]$TargetRoot = "D:\contextgraph\docker-desktop-data",
    [int]$DiskSizeMiB = 2097152,
    [string]$EvidencePath = "D:\contextgraph\logs\docker-desktop-d-migration-evidence.json",
    [switch]$StartDockerDesktop
)

$ErrorActionPreference = "Stop"

function Get-DirectorySizeBytes {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return 0
    }
    $sum = 0
    Get-ChildItem -LiteralPath $Path -Recurse -Force -File | ForEach-Object {
        $sum += $_.Length
    }
    return $sum
}

function Get-DriveFreeBytes {
    param([string]$DriveLetter)
    $drive = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$DriveLetter'"
    if ($null -eq $drive) {
        return $null
    }
    return [int64]$drive.FreeSpace
}

function Write-Utf8NoBomFile {
    param(
        [string]$Path,
        [string]$Text
    )
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Text, $encoding)
}

function Stop-DockerDesktop {
    $dockerCli = "C:\Program Files\Docker\Docker\DockerCli.exe"
    if (Test-Path -LiteralPath $dockerCli) {
        & $dockerCli -Shutdown | Out-Null
    }

    $deadline = (Get-Date).AddMinutes(5)
    do {
        $procs = Get-Process | Where-Object {
            $_.ProcessName -in @("Docker Desktop", "com.docker.backend", "com.docker.proxy", "vpnkit", "com.docker.service")
        }
        if ($procs.Count -eq 0) {
            return
        }
        Start-Sleep -Seconds 2
    } while ((Get-Date) -lt $deadline)

    throw "Docker Desktop processes did not exit within 5 minutes"
}

$userHome = [Environment]::GetFolderPath("UserProfile")
$sourceRoot = Join-Path $userHome "AppData\Local\Docker\wsl"
$settingsPath = Join-Path $userHome "AppData\Roaming\Docker\settings-store.json"
$dockerDesktopExe = "C:\Program Files\Docker\Docker\Docker Desktop.exe"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$sourceBackupRoot = "$sourceRoot.before-d-migration-$timestamp"
$settingsBackupPath = "$settingsPath.before-d-migration-$timestamp.bak"

$sourceExistsBefore = Test-Path -LiteralPath $sourceRoot
$targetHasPayloadBefore = (Test-Path -LiteralPath (Join-Path $TargetRoot "disk\docker_data.vhdx")) -or
    (Test-Path -LiteralPath (Join-Path $TargetRoot "main\ext4.vhdx"))

$before = [ordered]@{
    timestamp = (Get-Date).ToString("o")
    source_root = $sourceRoot
    target_root = $TargetRoot
    source_exists = $sourceExistsBefore
    target_exists = Test-Path -LiteralPath $TargetRoot
    source_size_bytes = Get-DirectorySizeBytes -Path $sourceRoot
    target_size_bytes = Get-DirectorySizeBytes -Path $TargetRoot
    target_has_payload = $targetHasPayloadBefore
    c_free_bytes = Get-DriveFreeBytes -DriveLetter "C:"
    d_free_bytes = Get-DriveFreeBytes -DriveLetter "D:"
}

if (-not $sourceExistsBefore -and -not $targetHasPayloadBefore) {
    throw "Docker Desktop WSL source root does not exist and target has no VHDX payload: source=$sourceRoot target=$TargetRoot"
}
if (-not (Test-Path -LiteralPath $settingsPath)) {
    throw "Docker Desktop settings-store.json does not exist: $settingsPath"
}

New-Item -ItemType Directory -Force -Path $TargetRoot | Out-Null
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $EvidencePath) | Out-Null

Stop-DockerDesktop

$targetHasPayload = (Test-Path -LiteralPath (Join-Path $TargetRoot "disk\docker_data.vhdx")) -or
    (Test-Path -LiteralPath (Join-Path $TargetRoot "main\ext4.vhdx"))
if (-not $targetHasPayload -and $sourceExistsBefore) {
    $robocopyArgs = @(
        $sourceRoot,
        $TargetRoot,
        "/MIR",
        "/COPY:DAT",
        "/DCOPY:DAT",
        "/R:2",
        "/W:2",
        "/J",
        "/NP"
    )
    & robocopy @robocopyArgs
    $robocopyExit = $LASTEXITCODE
    if ($robocopyExit -gt 7) {
        throw "robocopy failed with exit code $robocopyExit"
    }
} else {
    $robocopyExit = 0
}

if (-not (Test-Path -LiteralPath (Join-Path $TargetRoot "disk\docker_data.vhdx"))) {
    throw "target docker_data.vhdx missing after copy: $(Join-Path $TargetRoot 'disk\docker_data.vhdx')"
}
if (-not (Test-Path -LiteralPath (Join-Path $TargetRoot "main\ext4.vhdx"))) {
    throw "target ext4.vhdx missing after copy: $(Join-Path $TargetRoot 'main\ext4.vhdx')"
}

Copy-Item -LiteralPath $settingsPath -Destination $settingsBackupPath -Force
$settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
if (-not ($settings.PSObject.Properties.Name -contains "CustomWslDistroDir")) {
    $settings | Add-Member -NotePropertyName "CustomWslDistroDir" -NotePropertyValue $TargetRoot
} else {
    $settings.CustomWslDistroDir = $TargetRoot
}
if (-not ($settings.PSObject.Properties.Name -contains "DiskSizeMiB")) {
    $settings | Add-Member -NotePropertyName "DiskSizeMiB" -NotePropertyValue $DiskSizeMiB
} else {
    $settings.DiskSizeMiB = $DiskSizeMiB
}
$settingsJson = $settings | ConvertTo-Json -Depth 20
Write-Utf8NoBomFile -Path $settingsPath -Text ($settingsJson + [Environment]::NewLine)

$sourceBackupError = $null
if (Test-Path -LiteralPath $sourceRoot) {
    try {
        Rename-Item -LiteralPath $sourceRoot -NewName (Split-Path -Leaf $sourceBackupRoot)
    } catch {
        $sourceBackupError = $_.Exception.Message
    }
}

if ($StartDockerDesktop) {
    if (-not (Test-Path -LiteralPath $dockerDesktopExe)) {
        throw "Docker Desktop executable missing: $dockerDesktopExe"
    }
    Start-Process -FilePath $dockerDesktopExe | Out-Null
}

$after = [ordered]@{
    timestamp = (Get-Date).ToString("o")
    source_root_exists = Test-Path -LiteralPath $sourceRoot
    source_backup_root = $sourceBackupRoot
    source_backup_exists = Test-Path -LiteralPath $sourceBackupRoot
    target_root_exists = Test-Path -LiteralPath $TargetRoot
    target_size_bytes = Get-DirectorySizeBytes -Path $TargetRoot
    target_docker_data_vhdx_exists = Test-Path -LiteralPath (Join-Path $TargetRoot "disk\docker_data.vhdx")
    target_ext4_vhdx_exists = Test-Path -LiteralPath (Join-Path $TargetRoot "main\ext4.vhdx")
    settings_path = $settingsPath
    settings_backup_path = $settingsBackupPath
    settings_custom_wsl_distro_dir = (Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json).CustomWslDistroDir
    settings_disk_size_mib = (Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json).DiskSizeMiB
    robocopy_exit_code = $robocopyExit
    source_backup_error = $sourceBackupError
    c_free_bytes = Get-DriveFreeBytes -DriveLetter "C:"
    d_free_bytes = Get-DriveFreeBytes -DriveLetter "D:"
}

$evidence = [ordered]@{
    migration = "docker_desktop_wsl_to_d"
    before = $before
    after = $after
}
$evidenceJson = $evidence | ConvertTo-Json -Depth 10
Write-Utf8NoBomFile -Path $EvidencePath -Text ($evidenceJson + [Environment]::NewLine)
$evidenceJson

if ($sourceBackupError) {
    throw "Docker Desktop storage was copied and settings were updated, but source root backup rename failed: $sourceBackupError"
}
