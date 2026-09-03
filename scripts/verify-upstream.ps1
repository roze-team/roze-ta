$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$manifest = Get-Content -LiteralPath (Join-Path $projectRoot "UPSTREAM.json") -Raw -Encoding UTF8 | ConvertFrom-Json
$vendorRoot = Join-Path $projectRoot "vendor/yata"
$files = @(Get-ChildItem -LiteralPath $vendorRoot -File -Recurse -Force)
$expected = @($manifest.files.PSObject.Properties)
if ($files.Count -ne $expected.Count) { throw "Upstream file count changed" }
$sha = [System.Security.Cryptography.SHA256]::Create()
try { foreach ($entry in $expected) {
    $filePath = Join-Path $vendorRoot $entry.Name
    if (!(Test-Path -LiteralPath $filePath)) { throw "Missing upstream file: $($entry.Name)" }
    $digest = [BitConverter]::ToString($sha.ComputeHash([System.IO.File]::ReadAllBytes($filePath))).Replace("-", "").ToLowerInvariant()
    if ($digest -ne $entry.Value) {
        throw "Upstream differs: $($entry.Name)"
    }
} } finally { $sha.Dispose() }
Write-Output "Verified $($files.Count) upstream files at $($manifest.commit)"
