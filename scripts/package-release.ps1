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
    # Compress-Archive includes the enclosing fpdf directory for portable use.
    Compress-Archive -LiteralPath $taskDist -DestinationPath $taskZip -CompressionLevel Optimal -Force
    $taskPdf = Join-Path $taskRoot "dist/fpdf-$taskVersion-user-guide-zh-CN.pdf"
    Copy-Item -LiteralPath $taskManual -Destination $taskPdf -Force
    $taskAssets = @("$taskRoot/dist/fpdf-$taskVersion-windows-x64-setup.exe", $taskZip, $taskPdf)
    $taskSums = foreach ($taskAsset in $taskAssets) {
        (Get-FileHash -LiteralPath $taskAsset -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + (Split-Path -Leaf $taskAsset)
    }
    [System.IO.File]::WriteAllText((Join-Path $taskRoot 'dist/SHA256SUMS.txt'), ($taskSums -join "`n") + "`n", (New-Object System.Text.UTF8Encoding($false)))
    Write-Output "Release files prepared in $taskRoot/dist"
} finally { Pop-Location }
