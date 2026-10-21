$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
$taskRuntime = Join-Path $taskRepoRoot 'runtime'
$taskDownloads = Join-Path $taskRepoRoot 'analysis/tools'
$taskArchive = Join-Path $taskDownloads 'pdfium-win-x64.tgz'
$taskUrl = 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8111/pdfium-win-x64.tgz'
$taskSha256 = '3452d11031050ec65e57f180215b57b9e6d465e3fbedafa8a77ad90885c7d23e'
New-Item -ItemType Directory -Path $taskRuntime, $taskDownloads -Force | Out-Null
if (!(Test-Path -LiteralPath $taskArchive) -or (Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLower() -ne $taskSha256) {
    & curl.exe -fLsS --retry 2 --connect-timeout 20 --max-time 180 -o $taskArchive $taskUrl
    if ($LASTEXITCODE -ne 0) { throw 'PDFium download failed.' }
}
if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLower() -ne $taskSha256) {
    throw 'PDFium archive SHA256 mismatch.'
}
& tar.exe -xzf $taskArchive -C $taskRuntime
if ($LASTEXITCODE -ne 0) { throw 'PDFium extraction failed.' }
@{ version = 'chromium/8111'; url = $taskUrl; sha256 = $taskSha256 } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRuntime 'pdfium-source.json') -Encoding UTF8
Write-Output "PDFium ready: $taskRuntime/bin/pdfium.dll"
