param([string]$IsccPath = 'C:/Program Files (x86)/Inno Setup 6/ISCC.exe')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    $taskVersion = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
    if (!$taskVersion) { throw 'Missing application version.' }
    $taskDist = Join-Path $taskRoot 'dist/fpdf'
    $taskManual = Join-Path $taskRoot "output/pdf/fpdf-$taskVersion-user-guide-zh-CN.pdf"
    foreach ($taskRequired in @("$taskDist/fpdf.exe", "$taskDist/pdfium.dll", "$taskDist/ai/node.exe", "$taskDist/ai/bridge.mjs", "$taskDist/THIRD_PARTY_NOTICES.txt", $taskManual, $IsccPath)) {
        if (!(Test-Path -LiteralPath $taskRequired -PathType Leaf)) { throw "Required file missing: $taskRequired" }
    }
    # A viewport icon does not supply Windows Explorer/taskbar shortcut icons.
    # Refuse to package a PE without embedded RT_ICON and RT_GROUP_ICON resources.
    $taskExe = Join-Path $taskDist 'fpdf.exe'
    $taskBinary = [System.IO.File]::OpenRead($taskExe)
    $taskReader = New-Object System.IO.BinaryReader($taskBinary)
    try {
        $taskBinary.Position = 0x3c
        $taskPeOffset = $taskReader.ReadUInt32()
        $taskBinary.Position = $taskPeOffset
        if ($taskReader.ReadUInt32() -ne 0x00004550) { throw 'Application is not a Windows PE executable.' }
        $taskBinary.Position = $taskPeOffset + 6
        $taskSectionCount = $taskReader.ReadUInt16()
        $taskBinary.Position = $taskPeOffset + 20
        $taskOptionalSize = $taskReader.ReadUInt16()
        $taskOptionalOffset = $taskPeOffset + 24
        $taskBinary.Position = $taskOptionalOffset
        $taskPeMagic = $taskReader.ReadUInt16()
        $taskDataOffset = if ($taskPeMagic -eq 0x20b) { 112 } elseif ($taskPeMagic -eq 0x10b) { 96 } else { throw 'Unknown PE format.' }
        $taskBinary.Position = $taskOptionalOffset + $taskDataOffset + 16
        $taskResourceRva = $taskReader.ReadUInt32()
        $taskResourceOffset = $null
        for ($taskIndex = 0; $taskIndex -lt $taskSectionCount; $taskIndex++) {
            $taskBinary.Position = $taskOptionalOffset + $taskOptionalSize + 40 * $taskIndex + 8
            $taskVirtualSize = $taskReader.ReadUInt32()
            $taskVirtualAddress = $taskReader.ReadUInt32()
            $taskRawSize = $taskReader.ReadUInt32()
            $taskRawOffset = $taskReader.ReadUInt32()
            if ($taskResourceRva -ge $taskVirtualAddress -and $taskResourceRva -lt ($taskVirtualAddress + [Math]::Max($taskVirtualSize, $taskRawSize))) {
                $taskResourceOffset = $taskRawOffset + $taskResourceRva - $taskVirtualAddress
            }
        }
        if ($null -eq $taskResourceOffset) { throw 'Windows icon resource section missing.' }
        $taskBinary.Position = $taskResourceOffset + 12
        $taskNamedCount = $taskReader.ReadUInt16()
        $taskIdCount = $taskReader.ReadUInt16()
        $taskResourceIds = @()
        for ($taskIndex = 0; $taskIndex -lt $taskNamedCount + $taskIdCount; $taskIndex++) {
            $taskBinary.Position = $taskResourceOffset + 16 + 8 * $taskIndex
            $taskResourceIds += $taskReader.ReadUInt32()
        }
        if (3 -notin $taskResourceIds -or 14 -notin $taskResourceIds) { throw 'Application P icon is not embedded in the Windows executable; rebuild before packaging.' }
    } finally { $taskReader.Dispose(); $taskBinary.Dispose() }
    $taskFileVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($taskExe).ProductVersion
    if ($taskFileVersion -ne $taskVersion) { throw 'Executable version does not match Cargo.toml; rebuild before packaging.' }
    Copy-Item -LiteralPath './docs/usage.md' -Destination "$taskDist/README.md" -Force
    Copy-Item -LiteralPath './agent.example.txt' -Destination $taskDist -Force
    Copy-Item -LiteralPath $taskManual -Destination "$taskDist/fpdf-user-guide-zh-CN.pdf" -Force
    $taskPrivateFiles = @(Get-ChildItem -LiteralPath $taskDist -File -Recurse | Where-Object {
        $_.Name -in @('agent.txt', 'githubtoken.txt', '.env') -or $_.Name -like '.env.*'
    })
    if ($taskPrivateFiles.Count) { throw 'Private configuration found in package directory; remove it before packaging.' }
    & $IsccPath "/DAppVersion=$taskVersion" './installer/fpdf.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    $taskZip = Join-Path $taskRoot "dist/fpdf-$taskVersion-windows-x64.zip"
    # Read shared runtime files without locking an existing reader's Node process.
    Add-Type -AssemblyName System.IO.Compression
    $taskZipStream = [System.IO.File]::Open($taskZip, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try {
        $taskArchive = New-Object System.IO.Compression.ZipArchive($taskZipStream, [System.IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($taskFile in Get-ChildItem -LiteralPath $taskDist -File -Recurse | Sort-Object FullName) {
                $taskRelative = $taskFile.FullName.Substring($taskDist.Length + 1).Replace('\', '/')
                $taskEntry = $taskArchive.CreateEntry("fpdf/$taskRelative", [System.IO.Compression.CompressionLevel]::Optimal)
                $taskSourceStream = [System.IO.File]::Open($taskFile.FullName, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, ([System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete))
                try {
                    $taskEntryStream = $taskEntry.Open()
                    try { $taskSourceStream.CopyTo($taskEntryStream) } finally { $taskEntryStream.Dispose() }
                } finally { $taskSourceStream.Dispose() }
            }
        } finally { $taskArchive.Dispose() }
    } finally { $taskZipStream.Dispose() }
    $taskPdf = Join-Path $taskRoot "dist/fpdf-$taskVersion-user-guide-zh-CN.pdf"
    Copy-Item -LiteralPath $taskManual -Destination $taskPdf -Force
    $taskAssets = @("$taskRoot/dist/fpdf-$taskVersion-windows-x64-setup.exe", $taskZip, $taskPdf)
    $taskSums = foreach ($taskAsset in $taskAssets) {
        (Get-FileHash -LiteralPath $taskAsset -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + (Split-Path -Leaf $taskAsset)
    }
    [System.IO.File]::WriteAllText((Join-Path $taskRoot 'dist/SHA256SUMS.txt'), ($taskSums -join "`n") + "`n", (New-Object System.Text.UTF8Encoding($false)))
    Write-Output "Release files prepared in $taskRoot/dist"
} finally { Pop-Location }
