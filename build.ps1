#!/usr/bin/env pwsh
# build.ps1 - Full build with quality checks
# Exit codes: 0=success, 1=test failure, 2=clippy failure,
#   3=coverage failure, 4=build failure, 5=e2e failure

param(
    [Parameter(Position = 0)]
    [ValidateSet(
        "build", "build-only", "test", "clippy",
        "coverage", "validate", "e2e", "frontend",
        "clean", "help"
    )]
    [string]$Command = "build",
    [switch]$Help
)

if ($Help -or $Command -eq "help") {
    Write-Host @"
Usage: .\build.ps1 [command]

Commands:
  build       Full build with all quality checks (default)
  build-only  Build release binaries only
  test        Run all Rust tests
  clippy      Run clippy linter
  coverage    Generate HTML coverage report
  validate    Run cargo xtask validate
  e2e         Run Playwright end-to-end tests
  frontend    Build frontend (npm run build)
  clean       Clean build artifacts
  help        Show this help
"@
    exit 0
}

function Invoke-Build {
    Invoke-Validate
    Invoke-BuildOnly
    Invoke-Frontend
    Write-Host "Build OK"
}

function Invoke-BuildOnly {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { exit 4 }
}

function Invoke-Test {
    cargo xtask test
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

function Invoke-Clippy {
    cargo xtask clippy
    if ($LASTEXITCODE -ne 0) { exit 2 }
}

function Invoke-Coverage {
    cargo llvm-cov --workspace --html
    if ($LASTEXITCODE -ne 0) { exit 3 }
    Write-Host "Coverage: target/llvm-cov/html/index.html"
}

function Invoke-Validate {
    cargo xtask validate
    if ($LASTEXITCODE -ne 0) { exit 3 }
}

function Invoke-E2E {
    npx playwright test
    if ($LASTEXITCODE -ne 0) { exit 5 }
}

function Invoke-Frontend {
    Push-Location frontend
    try {
        npm run build
        if ($LASTEXITCODE -ne 0) { exit 4 }
    }
    finally {
        Pop-Location
    }
}

function Invoke-Clean {
    cargo clean
    foreach ($f in @(
        "coverage.xml", "coverage.json"
    )) {
        if (Test-Path $f) { Remove-Item $f }
    }
    if (Test-Path "frontend/dist") {
        Remove-Item -Recurse "frontend/dist"
    }
    Write-Host "Clean OK"
}

switch ($Command) {
    "build"      { Invoke-Build }
    "build-only" { Invoke-BuildOnly }
    "test"       { Invoke-Test }
    "clippy"     { Invoke-Clippy }
    "coverage"   { Invoke-Coverage }
    "validate"   { Invoke-Validate }
    "e2e"        { Invoke-E2E }
    "frontend"   { Invoke-Frontend }
    "clean"      { Invoke-Clean }
}
