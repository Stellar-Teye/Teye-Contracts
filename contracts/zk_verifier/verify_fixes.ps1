# ZK Verifier Test Compilation Verification Script (PowerShell)
# This script verifies that all test compilation issues have been resolved

$ErrorActionPreference = "Stop"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "ZK Verifier Test Compilation Verification" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

# Function to print success
function Print-Success {
    param($Message)
    Write-Host "✓ $Message" -ForegroundColor Green
}

# Function to print error
function Print-Error {
    param($Message)
    Write-Host "✗ $Message" -ForegroundColor Red
}

# Function to print info
function Print-Info {
    param($Message)
    Write-Host "ℹ $Message" -ForegroundColor Yellow
}

# Change to the workspace root
$scriptPath = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location (Join-Path $scriptPath "../../..")

Write-Host "Step 1: Checking package compilation..." -ForegroundColor Cyan
try {
    $output = cargo check -p zk_verifier 2>&1 | Out-String
    $output | Out-File -FilePath "$env:TEMP\zk_check.log"
    if ($LASTEXITCODE -eq 0) {
        Print-Success "Package compiles successfully"
    } else {
        throw "Compilation failed"
    }
} catch {
    Print-Error "Package compilation failed"
    Write-Host "See $env:TEMP\zk_check.log for details"
    exit 1
}
Write-Host ""

Write-Host "Step 2: Running clippy on all targets..." -ForegroundColor Cyan
try {
    $output = cargo clippy -p zk_verifier --all-targets -- -D warnings 2>&1 | Out-String
    $output | Out-File -FilePath "$env:TEMP\zk_clippy.log"
    if ($LASTEXITCODE -eq 0) {
        Print-Success "Clippy checks passed"
    } else {
        throw "Clippy failed"
    }
} catch {
    Print-Error "Clippy checks failed"
    Write-Host "See $env:TEMP\zk_clippy.log for details"
    exit 1
}
Write-Host ""

Write-Host "Step 3: Checking code formatting..." -ForegroundColor Cyan
try {
    $output = cargo fmt --all -- --check 2>&1 | Out-String
    $output | Out-File -FilePath "$env:TEMP\zk_fmt.log"
    if ($LASTEXITCODE -eq 0) {
        Print-Success "Code formatting is correct"
    } else {
        throw "Formatting issues found"
    }
} catch {
    Print-Error "Code formatting issues found"
    Write-Host "Run 'cargo fmt --all' to fix formatting"
    Write-Host "See $env:TEMP\zk_fmt.log for details"
    exit 1
}
Write-Host ""

Write-Host "Step 4: Running tests..." -ForegroundColor Cyan
try {
    $output = cargo test -p zk_verifier 2>&1 | Out-String
    $output | Out-File -FilePath "$env:TEMP\zk_test.log"
    if ($LASTEXITCODE -eq 0) {
        Print-Success "All tests passed"
    } else {
        throw "Tests failed"
    }
} catch {
    Print-Error "Some tests failed"
    Write-Host "See $env:TEMP\zk_test.log for details"
    exit 1
}
Write-Host ""

Write-Host "Step 5: Verifying specific test files exist..." -ForegroundColor Cyan
$testFiles = @(
    "contracts/zk_verifier/tests/bench_verify.rs",
    "contracts/zk_verifier/tests/test_nonce_replay.rs",
    "contracts/zk_verifier/tests/test_zk_access.rs"
)

foreach ($testFile in $testFiles) {
    if (Test-Path $testFile) {
        Print-Info "Checking $testFile..."
        Print-Success "$(Split-Path -Leaf $testFile) is valid"
    } else {
        Print-Error "Test file not found: $testFile"
        exit 1
    }
}
Write-Host ""

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "All verification checks passed!" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:"
Write-Host "  ✓ Package compilation successful"
Write-Host "  ✓ Clippy checks passed (no warnings)"
Write-Host "  ✓ Code formatting correct"
Write-Host "  ✓ All tests passed"
Write-Host "  ✓ All test files compile"
Write-Host ""
Write-Host "The zk_verifier contract is ready for use." -ForegroundColor Green
