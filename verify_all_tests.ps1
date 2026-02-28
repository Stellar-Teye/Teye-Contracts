# Comprehensive Test Verification Script (PowerShell)
# Verifies all packages affected by zk_verifier export fixes

$ErrorActionPreference = "Continue"

function Print-Success {
    param($Message)
    Write-Host "✓ $Message" -ForegroundColor Green
}

function Print-Error {
    param($Message)
    Write-Host "✗ $Message" -ForegroundColor Red
}

function Print-Info {
    param($Message)
    Write-Host "ℹ $Message" -ForegroundColor Yellow
}

function Print-Header {
    param($Message)
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host $Message -ForegroundColor Cyan
    Write-Host "========================================" -ForegroundColor Cyan
}

$packages = @("zk_verifier", "zk_voting", "zk_prover", "identity")
$failed = 0

Print-Header "Verifying All Test Compilations"
Write-Host ""

foreach ($package in $packages) {
    Print-Info "Checking package: $package"
    
    # Check compilation
    try {
        $output = cargo check -p $package --all-targets 2>&1 | Out-String
        $output | Out-File -FilePath "$env:TEMP\${package}_check.log"
        if ($LASTEXITCODE -eq 0) {
            Print-Success "$package`: Compilation successful"
        } else {
            throw "Compilation failed"
        }
    } catch {
        Print-Error "$package`: Compilation failed"
        Write-Host "See $env:TEMP\${package}_check.log for details"
        $failed++
    }
    
    # Run clippy
    try {
        $output = cargo clippy -p $package --all-targets -- -D warnings 2>&1 | Out-String
        $output | Out-File -FilePath "$env:TEMP\${package}_clippy.log"
        if ($LASTEXITCODE -eq 0) {
            Print-Success "$package`: Clippy checks passed"
        } else {
            throw "Clippy failed"
        }
    } catch {
        Print-Error "$package`: Clippy checks failed"
        Write-Host "See $env:TEMP\${package}_clippy.log for details"
        $failed++
    }
    
    Write-Host ""
}

Print-Header "Summary"
Write-Host ""

if ($failed -eq 0) {
    Print-Success "All packages verified successfully!"
    Write-Host ""
    Write-Host "Packages checked:"
    foreach ($package in $packages) {
        Write-Host "  ✓ $package"
    }
    Write-Host ""
    Write-Host "You can now run tests with:"
    Write-Host "  cargo test -p zk_verifier"
    Write-Host "  cargo test -p zk_voting"
    Write-Host "  cargo test -p zk_prover"
    Write-Host "  cargo test -p identity"
    exit 0
} else {
    Print-Error "$failed package(s) failed verification"
    Write-Host ""
    Write-Host "Check the log files in $env:TEMP for details"
    exit 1
}
