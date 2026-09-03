$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$items = Import-Csv -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/indicator-backlog.csv')
if ($items.Count -ne 84 -or @($items.id | Sort-Object -Unique).Count -ne 84) { throw 'Expected 84 unique capability IDs' }
foreach ($group in @(@('indicator',60),@('pattern',12),@('feature',12))) {
    if (@($items | Where-Object { $_.kind -eq $group[0] }).Count -ne $group[1]) { throw "Incorrect count: $($group[0])" }
}
if (@($items | Where-Object { $_.batch -eq 'A' }).Count -ne 40 -or @($items | Where-Object { $_.batch -eq 'B' }).Count -ne 20) { throw 'A/B batch count mismatch' }
$coverage = Import-Csv -Encoding UTF8 -LiteralPath (Join-Path $project 'docs/upstream-indicator-coverage.csv')
$source = Get-Content -Raw -Encoding UTF8 -LiteralPath (Join-Path $project 'vendor/yata/src/indicators/mod.rs')
$modules = @([regex]::Matches($source,'(?m)^mod ([a-z_]+);') | ForEach-Object { $_.Groups[1].Value } | Sort-Object)
if ($modules.Count -ne 36 -or $coverage.Count -ne 36 -or @($coverage.module | Sort-Object -Unique).Count -ne 36) { throw 'Expected 36 distinct actual upstream modules' }
if (Compare-Object $modules ($coverage.module | Sort-Object)) { throw 'Coverage differs from actual Yata source modules' }
foreach ($row in $coverage) {
    if (-not $row.reason -or -not $row.status) { throw "Missing decision for $($row.module)" }
    if ($row.target_id -and $row.target_id -notin $items.id) { throw "Unknown target $($row.target_id)" }
}
Write-Output 'Verified 84 unique targets (60+12+12), A=40/B=20, and all 36 upstream module decisions.'
