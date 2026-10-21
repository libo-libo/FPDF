# Generate release notices from the locked Windows dependency graph and the
# original license files shipped in Cargo's source packages. No network fetches.
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
if (Test-Path -LiteralPath (Join-Path $taskRoot 'analysis/tools/rust/cargo/bin/cargo.exe')) {
    . (Join-Path $PSScriptRoot 'dev-env.ps1')
}
$taskTarget = $env:CARGO_BUILD_TARGET
if (!$taskTarget) {
    $taskHostLine = (& rustc -vV | Select-String '^host:').Line
    if ($LASTEXITCODE -ne 0 -or !$taskHostLine) { throw 'Cannot determine Rust target.' }
    $taskTarget = $taskHostLine.Substring(6).Trim()
}
Push-Location $taskRoot
try {
    $taskMetadataJson = & cargo metadata --format-version 1 --filter-platform $taskTarget --locked
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $taskMetadata = ($taskMetadataJson -join "`n") | ConvertFrom-Json
    # metadata includes unselected optional packages. Restrict notices to the
    # normal/build graph selected by this project's current features and target.
    $taskTree = & cargo tree --locked --target $taskTarget -e normal,build --prefix none --format '{p}'
    if ($LASTEXITCODE -ne 0) { throw 'cargo tree failed' }
    $taskSelected = @{}
    foreach ($taskLine in $taskTree) {
        if ($taskLine -match '^([^ ]+) v([^ ]+)') { $taskSelected[$Matches[1] + '@' + $Matches[2]] = $true }
    }
    $taskPackages = @($taskMetadata.packages | Where-Object {
        $_.id -ne $taskMetadata.resolve.root -and $taskSelected.ContainsKey($_.name + '@' + $_.version)
    } | Sort-Object name,version)
    $taskSupplementRoot = Join-Path $taskRoot 'third-party/licenses'
    $taskSources = Get-Content -Raw -LiteralPath (Join-Path $taskSupplementRoot 'sources.json') | ConvertFrom-Json
    foreach ($taskSource in $taskSources) {
        $taskSourcePath = Join-Path (Join-Path $taskSupplementRoot $taskSource.package) $taskSource.file
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $taskSourcePath).Hash.ToLowerInvariant() -ne $taskSource.sha256) {
            throw "Supplement license digest differs from provenance: $taskSourcePath"
        }
    }
    $taskText = New-Object System.Text.StringBuilder
    [void]$taskText.AppendLine('THIRD-PARTY NOTICES - fpdf')
    [void]$taskText.AppendLine("Target: $taskTarget; Cargo.lock and default project features.")
    [void]$taskText.AppendLine('Includes normal and build dependencies; some build-only code may not be linked.')
    [void]$taskText.AppendLine('Package SPDX expressions are recorded verbatim. Original license texts follow.')
    [void]$taskText.AppendLine('PDFium and its components: see PDFium-LICENSE.txt and licenses/ in this release.')
    [void]$taskText.AppendLine('Local fpdf code: see LICENSE. System fonts are loaded from Windows, not redistributed.')
    [void]$taskText.AppendLine("Dependency package count: $($taskPackages.Count)")
    foreach ($taskPackage in $taskPackages) {
        $taskPackageRoot = Split-Path -Parent $taskPackage.manifest_path
        $taskFiles = @{}
        # Recursive collection retains egui's bundled font terms (including
        # Hack-Regular.txt, OFL.txt and UFL.txt), as well as nested notices.
        foreach ($taskFile in Get-ChildItem -LiteralPath $taskPackageRoot -Recurse -File) {
            if ($taskFile.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE|COPYRIGHT)([._-]|$)' -or
                $taskFile.Name -match '^(OFL|UFL)\.txt$' -or
                ($taskPackage.name -eq 'epaint_default_fonts' -and $taskFile.Directory.Name -eq 'fonts' -and $taskFile.Extension -eq '.txt')) {
                $taskRelative = $taskFile.FullName.Substring($taskPackageRoot.Length + 1).Replace('\','/')
                $taskFiles[$taskRelative] = $taskFile.FullName
            }
        }
        if ($taskPackage.license_file) {
            $taskLicensePath = Join-Path $taskPackageRoot $taskPackage.license_file
            if (!(Test-Path -LiteralPath $taskLicensePath -PathType Leaf)) { throw "Missing declared license_file: $taskLicensePath" }
            $taskFiles[$taskPackage.license_file] = $taskLicensePath
        }
        $taskSupplement = $taskPackage.name + '-' + $taskPackage.version
        if ($taskPackage.version -eq '0.33.3' -and $taskPackage.name -in @('ecolor','eframe','egui','egui-winit','egui_glow','emath','epaint','epaint_default_fonts')) {
            $taskSupplement = 'egui-0.33.3'
        }
        foreach ($taskSource in @($taskSources | Where-Object { $_.package -eq $taskSupplement })) {
            $taskFiles['upstream/' + $taskSource.file] = Join-Path (Join-Path $taskSupplementRoot $taskSource.package) $taskSource.file
        }
        if (!$taskFiles.Count) { throw "No original license files found for $($taskPackage.name) $($taskPackage.version)" }
        [void]$taskText.AppendLine("`n======================================================================")
        [void]$taskText.AppendLine("Package: $($taskPackage.name) $($taskPackage.version)")
        [void]$taskText.AppendLine("SPDX: $($taskPackage.license)")
        [void]$taskText.AppendLine("Repository: $($taskPackage.repository)")
        foreach ($taskLabel in @($taskFiles.Keys | Sort-Object)) {
            [void]$taskText.AppendLine("`n--- $taskLabel ---")
            if ($taskLabel.StartsWith('upstream/')) {
                $taskSource = $taskSources | Where-Object { $_.package -eq $taskSupplement -and $_.file -eq $taskLabel.Substring(9) } | Select-Object -First 1
                [void]$taskText.AppendLine("Source: $($taskSource.url)")
                [void]$taskText.AppendLine("SHA-256: $($taskSource.sha256)")
            }
            [void]$taskText.AppendLine([System.IO.File]::ReadAllText($taskFiles[$taskLabel]))
        }
    }
    $taskOutputDirectory = Join-Path $taskRoot 'dist/fpdf'
    [void][System.IO.Directory]::CreateDirectory($taskOutputDirectory)
    $taskOutput = Join-Path $taskOutputDirectory 'THIRD_PARTY_NOTICES.txt'
    [System.IO.File]::WriteAllText($taskOutput, $taskText.ToString(), (New-Object System.Text.UTF8Encoding($false)))
    Write-Host "Collected $($taskPackages.Count) dependency packages: $taskOutput"
} finally {
    Pop-Location
}
