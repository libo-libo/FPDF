param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRepoRoot
try {
    if (Test-Path -LiteralPath './analysis/tools/rust/cargo/bin/cargo.exe') {
        . "$PSScriptRoot/dev-env.ps1"
    }
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install stable Rust or use the project-local toolchain.' }
    if (!(Test-Path -LiteralPath './runtime/bin/pdfium.dll')) { & "$PSScriptRoot/fetch-pdfium.ps1" }
    if (!(Test-Path -LiteralPath './runtime/ai/bridge.mjs') -or !(Test-Path -LiteralPath './runtime/ai/node.exe')) {
        throw 'Prepare AI runtime: cd ai-runtime; npm ci; cd ..; node scripts/build-ai.mjs'
    }
    & cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    if (!$SkipTests) {
        & cargo test --locked
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    }
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    $taskTargetDir = if ($env:CARGO_BUILD_TARGET) { "target/$env:CARGO_BUILD_TARGET/release" } else { 'target/release' }
    $taskDist = Join-Path $taskRepoRoot 'dist/fpdf'
    New-Item -ItemType Directory -Path $taskDist -Force | Out-Null
    # Avoid rewriting identical binaries that an existing reader may have loaded.
    foreach ($taskBinary in @("$taskTargetDir/fpdf.exe", './runtime/bin/pdfium.dll', './LICENSE')) {
        $taskDestination = Join-Path $taskDist (Split-Path -Leaf $taskBinary)
        if (!(Test-Path -LiteralPath $taskDestination) -or
            (Get-FileHash -LiteralPath $taskBinary).Hash -ne (Get-FileHash -LiteralPath $taskDestination).Hash) {
            Copy-Item -LiteralPath $taskBinary -Destination $taskDestination
        }
    }
    Copy-Item -LiteralPath './docs/usage.md' -Destination "$taskDist/README.md"
    Copy-Item -LiteralPath './runtime/LICENSE' -Destination "$taskDist/PDFium-LICENSE.txt"
    Copy-Item -LiteralPath './runtime/licenses' -Destination $taskDist -Recurse -Force
    Copy-Item -LiteralPath './runtime/VERSION','./runtime/pdfium-source.json' -Destination $taskDist
    $taskAiRoot = (Resolve-Path -LiteralPath './runtime/ai').Path
    foreach ($taskAiFile in Get-ChildItem -LiteralPath $taskAiRoot -File -Recurse) {
        $taskAiRelative = $taskAiFile.FullName.Substring($taskAiRoot.Length + 1)
        $taskAiDestination = Join-Path (Join-Path $taskDist 'ai') $taskAiRelative
        New-Item -ItemType Directory -Path (Split-Path -Parent $taskAiDestination) -Force | Out-Null
        if (!(Test-Path -LiteralPath $taskAiDestination) -or
            (Get-FileHash -LiteralPath $taskAiFile.FullName).Hash -ne (Get-FileHash -LiteralPath $taskAiDestination).Hash) {
            Copy-Item -LiteralPath $taskAiFile.FullName -Destination $taskAiDestination
        }
    }
    & (Join-Path $PSScriptRoot 'collect-notices.ps1')
    Write-Output "Ready: $taskDist/fpdf.exe"
} finally { Pop-Location }
