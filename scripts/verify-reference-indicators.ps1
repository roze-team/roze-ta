$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Get-Sha256($path) {
    $hash=[Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($hash.ComputeHash([IO.File]::ReadAllBytes($path))).Replace('-','').ToLowerInvariant() } finally { $hash.Dispose() }
}
function Assert-Blob($relativePath, $expected) {
    $bytes = [IO.File]::ReadAllBytes((Join-Path $project $relativePath))
    $prefix = $utf8.GetBytes(('blob ' + $bytes.Length + [char]0))
    $sha = [Security.Cryptography.SHA1]::Create()
    try { $actual = [BitConverter]::ToString($sha.ComputeHash([byte[]]($prefix + $bytes))).Replace('-', '').ToLowerInvariant() } finally { $sha.Dispose() }
    if ($actual -ne $expected) { throw "Original Git blob mismatch: $relativePath ($actual != $expected)" }
}
$baseline = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/evidence/reference-r1-source.json') | ConvertFrom-Json
foreach ($file in $baseline.files) { Assert-Blob $file.original $file.git_blob_sha }
Assert-Blob $baseline.license_file.path $baseline.license_file.git_blob_sha
$sources = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/reference-indicator-sources.json') | ConvertFrom-Json
$mapping = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/reference-indicator-mapping.json') | ConvertFrom-Json
$expected = @($sources.sources | ForEach-Object { $id=$_.id; $_.entries | ForEach-Object { $id + ':' + $_ } } | Sort-Object)
$actual = @($mapping.entries | ForEach-Object { $_.source + ':' + $_.name } | Sort-Object)
$referenceCatalog = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/evidence/reference-catalog.json') | ConvertFrom-Json
$referenceIds = @($referenceCatalog.entries | ForEach-Object { $_.id })
if ($expected.Count -ne $actual.Count -or @($actual | Sort-Object -Unique).Count -ne $actual.Count -or (Compare-Object $expected $actual)) { throw 'Source/mapping entries are missing or duplicated' }
foreach ($row in $mapping.entries) {
    if ($row.status -notin @('review_pending','implemented_variant','mapped_variant','verified_default_cases','verified_source_cases','verified_causal_cases','non_indicator')) { throw 'Unknown mapping status' }
    if ($row.status -in @('implemented_variant','mapped_variant','verified_default_cases','verified_source_cases','verified_causal_cases')) {
        if ($row.operation_id -notin $referenceIds -or -not $row.evidence -or -not (Test-Path -LiteralPath (Join-Path $project $row.evidence)) -or -not (Test-Path -LiteralPath (Join-Path $project $row.contract))) { throw 'Mapped entry has no registered operation/contract/evidence' }
        if ($row.status -eq 'mapped_variant' -and $row.compatibility -ne 'cross_library_numeric_parity_not_verified') { throw 'Cross-library availability must not imply numeric parity' }
        if ($row.status -eq 'verified_default_cases') {
            $parity = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project $row.evidence) | ConvertFrom-Json
            $cases = @($parity.records | Where-Object { $_.name -eq $row.name })
            if ($row.source -ne 'ta-lib' -or $cases.Count -ne 3 -or @($cases | Where-Object { $_.status -ne 'pass' }).Count -ne 0) { throw 'Default-case acceptance lacks three passing independent references' }
        }
        if ($row.status -in @('verified_source_cases','verified_causal_cases')) {
            $parity = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project $row.evidence) | ConvertFrom-Json
            $accepted = @($parity.entries | Where-Object { $_.source -eq $row.source -and $_.name -eq $row.name })
            if ($accepted.Count -ne 1 -or $accepted[0].operation_id -ne $row.operation_id -or $accepted[0].cases.Count -ne 3 -or @($accepted[0].cases | Where-Object { $_.differences -ne 0 }).Count -ne 0) { throw 'Source acceptance lacks three passing independent cases' }
            foreach ($fixture in $parity.fixture_sha256.PSObject.Properties) {
                if ((Get-Sha256 (Join-Path $project $fixture.Name)) -ne $fixture.Value) { throw 'Accepted fixture changed' }
            }
        }
    }
    if ($row.status -eq 'non_indicator' -and -not $row.reason) { throw 'Non-indicator classification needs a reason' }
}
$derivedManifest = Join-Path $project 'docs/evidence/reference-r1-derived.json'
if (Test-Path -LiteralPath $derivedManifest) {
    $derived = Get-Content -Raw -Encoding UTF8 -LiteralPath $derivedManifest | ConvertFrom-Json
    foreach ($file in $derived.files) {
        $hash=Get-Sha256 (Join-Path $project $file.path)
        if ($hash -ne $file.sha256) { throw "Derived source changed without a reviewed patch: $($file.path)" }
        $patchHash=Get-Sha256 (Join-Path $project $file.patch)
        if ($patchHash -ne $file.patch_sha256) { throw "Patch changed: $($file.patch)" }
    }
} else { throw 'Missing derived source manifest' }
$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$replay=Join-Path $tempRoot ('roze-ta-wickra-r1-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $replay | Out-Null
try {
    foreach ($file in $derived.files) {
        $target=Join-Path $replay $file.path
        New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
        Copy-Item -LiteralPath (Join-Path $project $file.original) -Destination $target
        Push-Location $replay
        try {
            rtk proxy git -c core.autocrlf=false apply --no-index --whitespace=error-all -- (Join-Path $project $file.patch)
            if ($LASTEXITCODE -ne 0) { throw "Patch replay failed: $($file.patch)" }
        } finally { Pop-Location }
        $hash=Get-Sha256 $target
        if ($hash -ne $file.sha256) { throw "Replayed source differs: $($file.path)" }
    }
} finally {
    $resolved=[IO.Path]::GetFullPath($replay)
    if (-not $resolved.StartsWith((Join-Path $tempRoot 'roze-ta-wickra-r1-'), [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected temporary cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
Write-Output ("Verified 9 original Rust blobs, license, derived hashes and " + $actual.Count + ' per-source coverage rows.')
