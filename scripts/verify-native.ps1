param([string]$ProjectRoot = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = [IO.Path]::GetFullPath($ProjectRoot).TrimEnd([IO.Path]::DirectorySeparatorChar)
function Resolve-ProjectFile([string]$relative) {
    $path = [IO.Path]::GetFullPath((Join-Path $root $relative))
    if (-not $path.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Path outside project: $relative"
    }
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing file: $relative" }
    return $path
}
function Digest([string]$relative) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [IO.File]::ReadAllBytes((Resolve-ProjectFile $relative))
        return [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Read-Json([string]$relative) {
    return Get-Content -Raw -Encoding UTF8 -LiteralPath (Resolve-ProjectFile $relative) | ConvertFrom-Json
}
$manifest = Read-Json 'docs/patches/native-migration.json'
if ($manifest.schema_version -ne 1) { throw 'Unsupported migration manifest version' }
if ((Digest 'UPSTREAM.json') -ne $manifest.upstream_manifest_sha256) { throw 'Original upstream manifest changed' }
$upstream = Read-Json 'UPSTREAM.json'
if ($upstream.commit -ne $manifest.upstream_commit) { throw 'Upstream commit mismatch' }
foreach ($entry in $upstream.files.PSObject.Properties) {
    if ((Digest ('vendor/yata/' + $entry.Name)) -ne $entry.Value) { throw "Original file differs: $($entry.Name)" }
}
$baselineFiles = @(Get-ChildItem -LiteralPath (Join-Path $root 'vendor/yata') -Recurse -Force -File)
if ($baselineFiles.Count -ne @($upstream.files.PSObject.Properties).Count) { throw 'Original file set changed' }
$expected = @($manifest.files | ForEach-Object { $_.target } | Sort-Object)
if (@($expected | Sort-Object -Unique).Count -ne $expected.Count) { throw 'Duplicate native target' }
$actual = @(foreach ($part in @('core','helpers','indicators','methods')) {
    Get-ChildItem -LiteralPath (Join-Path $root "crates/roze-ta/src/$part") -Recurse -File -Filter '*.rs' |
        ForEach-Object { $_.FullName.Substring($root.Length + 1).Replace('\','/') }
}) + @('crates/roze-ta/src/prelude.rs')
if (Compare-Object $expected ($actual | Sort-Object)) { throw 'Native source file set changed' }
$sourceNames = @($manifest.files | ForEach-Object { $_.source.Substring('vendor/yata/'.Length) } | Sort-Object)
$upstreamSources = @($upstream.files.PSObject.Properties.Name | Where-Object { $_ -like 'src/*.rs' } | Sort-Object)
if (Compare-Object $sourceNames $upstreamSources) { throw 'Incomplete upstream source mapping' }
foreach ($entry in $manifest.files) {
    if ((Digest $entry.source) -ne $entry.source_sha256) { throw "Source differs: $($entry.source)" }
    if ((Digest $entry.target) -ne $entry.target_sha256) { throw "Native file differs: $($entry.target)" }
    $text = [IO.File]::ReadAllText((Resolve-ProjectFile $entry.target))
    if (-not $text.Contains('SPDX-License-Identifier: Apache-2.0') -or
        -not $text.Contains('Copyright 2020 AMvDev') -or -not $text.Contains('Modified 2026')) {
        throw "Missing attribution or modification notice: $($entry.target)"
    }
}
foreach ($entry in $manifest.support_files.PSObject.Properties) {
    if ((Digest $entry.Name) -ne $entry.Value) {
        $additions = Read-Json 'docs/patches/native-notices-additions.json'
        $changes = @($additions.files | Where-Object { $_.path -ceq $entry.Name })
        if ($changes.Count -ne 1 -or (Digest $additions.baseline) -ne $entry.Value) { throw "Migration support file differs: $($entry.Name)" }
        $change = $changes[0]
        $original = [IO.File]::ReadAllText((Resolve-ProjectFile $additions.baseline))
        if (-not $original.Contains($change.replace_old)) { throw 'Notice replacement does not match original baseline' }
        $rebuiltNotice = $original.Replace($change.replace_old, $change.replace_new) + $change.append
        if ($rebuiltNotice -cne [IO.File]::ReadAllText((Resolve-ProjectFile $entry.Name))) { throw 'Notice additions do not reconstruct current file' }
    }
}
if ((Digest 'crates/roze-ta/LICENSE-APACHE') -ne (Digest 'vendor/yata/LICENSE')) { throw 'Apache license copy differs' }
$baseline = Read-Json 'docs/evidence/native-migration-baseline.json'
foreach ($entry in $baseline.files.PSObject.Properties) {
    if ((Digest $entry.Name) -ne $entry.Value) { throw "Pre-migration fixture differs: $($entry.Name)" }
}
if ((Digest $manifest.patch.path) -ne $manifest.patch.sha256) { throw 'Migration patch differs' }

# Apply each recorded unified diff in memory against the immutable original.
# This also rejects a target-hash-only update that hides an unrecorded change.
$patch = [IO.File]::ReadAllLines((Resolve-ProjectFile $manifest.patch.path))
$cursor = 0
foreach ($entry in $manifest.files) {
    if ($patch[$cursor] -cne ('--- ' + $entry.source) -or
        $patch[$cursor + 1] -cne ('+++ ' + $entry.target)) { throw 'Patch file mapping mismatch' }
    $cursor += 2
    $original = [IO.File]::ReadAllLines((Resolve-ProjectFile $entry.source))
    $rebuilt = [Collections.Generic.List[string]]::new()
    $oldIndex = 0
    while ($cursor -lt $patch.Count -and $patch[$cursor].StartsWith('@@ ')) {
        if ($patch[$cursor] -notmatch '^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@$') { throw 'Invalid patch hunk' }
        $oldStart = [Math]::Max(0, [int]$Matches[1] - 1)
        $oldCount = if ($Matches[2]) { [int]$Matches[2] } else { 1 }
        $newStart = [Math]::Max(0, [int]$Matches[3] - 1)
        $newCount = if ($Matches[4]) { [int]$Matches[4] } else { 1 }
        if ($oldStart -lt $oldIndex) { throw 'Overlapping patch hunks' }
        while ($oldIndex -lt $oldStart) { $rebuilt.Add($original[$oldIndex]); $oldIndex++ }
        if ($rebuilt.Count -ne $newStart) { throw 'Patch output offset mismatch' }
        $oldBefore = $oldIndex
        $newBefore = $rebuilt.Count
        $cursor++
        while ($cursor -lt $patch.Count -and -not $patch[$cursor].StartsWith('@@ ') -and -not $patch[$cursor].StartsWith('--- ')) {
            $line = $patch[$cursor]
            if ($line.Length -eq 0) { throw 'Invalid empty patch line' }
            $kind = $line.Substring(0,1)
            $value = $line.Substring(1)
            if ($kind -eq ' ' -or $kind -eq '-') {
                if ($oldIndex -ge $original.Count -or $original[$oldIndex] -cne $value) { throw "Patch source mismatch: $($entry.source)" }
                $oldIndex++
            }
            if ($kind -eq ' ' -or $kind -eq '+') { $rebuilt.Add($value) }
            if ($kind -notin @(' ', '-', '+')) { throw 'Unsupported patch record' }
            $cursor++
        }
        if ($oldIndex - $oldBefore -ne $oldCount -or $rebuilt.Count - $newBefore -ne $newCount) { throw 'Patch hunk size mismatch' }
    }
    while ($oldIndex -lt $original.Count) { $rebuilt.Add($original[$oldIndex]); $oldIndex++ }
    $expectedText = [string]::Join("`n", $rebuilt) + "`n"
    $actualText = [IO.File]::ReadAllText((Resolve-ProjectFile $entry.target)).Replace("`r`n", "`n")
    if ($expectedText -cne $actualText) { throw "Patch does not reconstruct native source: $($entry.target)" }
}
if ($cursor -ne $patch.Count) { throw 'Unmapped patch content' }
Write-Output "Verified $($expected.Count) native source mappings, replayed patches, licenses, and $($baseline.profiles) pre-migration snapshots."
