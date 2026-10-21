# Dot-source from PowerShell: . ./scripts/dev-env.ps1
# Changes apply only to this PowerShell process; no global PATH or registry edits.
$taskRepoRoot = Split-Path -Parent $PSScriptRoot
$taskRustRoot = Join-Path $taskRepoRoot 'analysis/tools/rust'
$taskToolchain = '1.99.1-x86_64-pc-windows-gnu'
$taskCargoBin = Join-Path $taskRustRoot 'cargo/bin'
$taskToolchainRoot = Join-Path $taskRustRoot "rustup/toolchains/$taskToolchain"
$taskMingwBin = Join-Path $taskToolchainRoot 'lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained'
$taskBinutilsBin = Join-Path $taskRustRoot 'binutils/mingw64/bin'
$taskImportLibDir = Join-Path $taskRustRoot 'import-libs'

if (!(Test-Path (Join-Path $taskCargoBin 'cargo.exe'))) {
    throw "Project-local Rust is missing. See analysis/tools/rust/TOOLCHAIN.md."
}
if (!(Test-Path (Join-Path $taskMingwBin 'x86_64-w64-mingw32-gcc.exe'))) {
    throw "Project-local Rust GNU linker is missing: $taskMingwBin"
}
if (!(Test-Path (Join-Path $taskBinutilsBin 'dlltool.exe'))) {
    throw "Project-local Binutils is missing: $taskBinutilsBin"
}
$env:RUSTUP_HOME = Join-Path $taskRustRoot 'rustup'
$env:CARGO_HOME = Join-Path $taskRustRoot 'cargo'
$env:RUSTUP_TOOLCHAIN = $taskToolchain
$env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-gnu'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $taskMingwBin 'x86_64-w64-mingw32-gcc.exe'
# Rust's bundled dlltool lacks a working assembler. Explicitly use the complete
# MSYS2 Binutils copy for raw-dylib import libraries (windows-link, parking_lot).
$taskDlltoolFlag = '-Cdlltool=' + (Join-Path $taskBinutilsBin 'dlltool.exe')
$taskRustFlags = @($env:CARGO_ENCODED_RUSTFLAGS -split [char]31 | Where-Object { $_ -and $_ -notlike '-Cdlltool=*' })
$env:CARGO_ENCODED_RUSTFLAGS = (@($taskRustFlags) + @($taskDlltoolFlag)) -join [char]31
# Supplemental Windows import libraries generated from this system's DLL exports.
# GCC consumes LIBRARY_PATH during linking; existing Cargo objects stay reusable.
$taskLibraryPaths = @($taskImportLibDir) + @($env:LIBRARY_PATH -split ';' | Where-Object { $_ })
$env:LIBRARY_PATH = ($taskLibraryPaths | Select-Object -Unique) -join ';'
$taskPathParts = @($taskCargoBin, $taskMingwBin, $taskBinutilsBin) + @($env:PATH -split ';')
$env:PATH = ($taskPathParts | Select-Object -Unique) -join ';'
