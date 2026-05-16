param(
    [string]$RepoRoot = (Resolve-Path ".").Path,
    [string]$ZedRoot = (Join-Path (Resolve-Path ".").Path "zed"),
    [string[]]$SeedCrates = @("gpui", "gpui_platform", "gpui_windows", "gpui_wgpu", "gpui_macros", "gpui_shared_string", "gpui_util"),
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$zedRootWasSupplied = $PSBoundParameters.ContainsKey("ZedRoot")

function Resolve-ExistingPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Resolve-Path -LiteralPath $Path).Path
}

function Test-IsUnderPath {
    param(
        [Parameter(Mandatory = $true)][string]$ChildPath,
        [Parameter(Mandatory = $true)][string]$ParentPath
    )

    $resolvedChild = [System.IO.Path]::GetFullPath($ChildPath)
    $resolvedParent = [System.IO.Path]::GetFullPath($ParentPath)

    if (-not $resolvedParent.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $resolvedParent = "$resolvedParent$([System.IO.Path]::DirectorySeparatorChar)"
    }

    return $resolvedChild.StartsWith($resolvedParent, [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-IsUnderPath {
    param(
        [Parameter(Mandatory = $true)][string]$ChildPath,
        [Parameter(Mandatory = $true)][string]$ParentPath,
        [Parameter(Mandatory = $true)][string]$Description
    )

    if (-not (Test-IsUnderPath -ChildPath $ChildPath -ParentPath $ParentPath)) {
        throw "$Description path '$ChildPath' is outside expected root '$ParentPath'."
    }
}

$RepoRoot = [System.IO.Path]::GetFullPath($RepoRoot)
$ZedRoot = [System.IO.Path]::GetFullPath($ZedRoot)
$zedCargoToml = Join-Path $ZedRoot "Cargo.toml"

if (-not (Test-Path -LiteralPath $zedCargoToml -PathType Leaf)) {
    if (-not $zedRootWasSupplied) {
        $repoRootInfo = [System.IO.DirectoryInfo]::new($RepoRoot)
        if (($null -ne $repoRootInfo.Parent) -and ($repoRootInfo.Parent.Name -eq ".worktrees") -and ($null -ne $repoRootInfo.Parent.Parent)) {
            $linkedWorktreeZedRoot = Join-Path $repoRootInfo.Parent.Parent.FullName "zed"
            $linkedWorktreeZedCargoToml = Join-Path $linkedWorktreeZedRoot "Cargo.toml"
            if (Test-Path -LiteralPath $linkedWorktreeZedCargoToml -PathType Leaf) {
                $ZedRoot = [System.IO.Path]::GetFullPath($linkedWorktreeZedRoot)
                $zedCargoToml = $linkedWorktreeZedCargoToml
            }
        }
    }
}

if (-not (Test-Path -LiteralPath $zedCargoToml -PathType Leaf)) {
    throw "Zed Cargo.toml was not found at '$zedCargoToml'. Pass -ZedRoot with the path to a Zed checkout."
}

$metadataJson = $null
Push-Location -LiteralPath $ZedRoot
try {
    $metadataJson = & cargo metadata --format-version=1 --no-deps
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed in '$ZedRoot'."
    }
} finally {
    Pop-Location
}

$metadata = $metadataJson | ConvertFrom-Json
$zedRootWithSeparator = $ZedRoot
if (-not $zedRootWithSeparator.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
    $zedRootWithSeparator = "$zedRootWithSeparator$([System.IO.Path]::DirectorySeparatorChar)"
}

$workspacePackagesByName = @{}
foreach ($package in $metadata.packages) {
    $manifestPath = [System.IO.Path]::GetFullPath($package.manifest_path)
    if ($manifestPath.StartsWith($zedRootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)) {
        $workspacePackagesByName[$package.name] = $package
    }
}

foreach ($seedCrate in $SeedCrates) {
    if (-not $workspacePackagesByName.ContainsKey($seedCrate)) {
        throw "Seed crate '$seedCrate' was not found in the Zed workspace metadata."
    }
}

$selectedPackagesByName = @{}
$queue = [System.Collections.Generic.Queue[string]]::new()
foreach ($seedCrate in $SeedCrates) {
    $queue.Enqueue($seedCrate)
}

while ($queue.Count -gt 0) {
    $crateName = $queue.Dequeue()
    if ($selectedPackagesByName.ContainsKey($crateName)) {
        continue
    }

    $package = $workspacePackagesByName[$crateName]
    if ($null -eq $package) {
        continue
    }

    $selectedPackagesByName[$crateName] = $package

    foreach ($dependency in $package.dependencies) {
        if ($dependency.kind -eq "dev") {
            continue
        }

        if ($workspacePackagesByName.ContainsKey($dependency.name) -and -not $selectedPackagesByName.ContainsKey($dependency.name)) {
            $queue.Enqueue($dependency.name)
        }
    }
}

$selectedRows = foreach ($package in ($selectedPackagesByName.Values | Sort-Object -Property name)) {
    $manifestPath = [System.IO.Path]::GetFullPath($package.manifest_path)
    $sourceDir = Split-Path -Parent $manifestPath
    if ($sourceDir.StartsWith($zedRootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)) {
        $relativeSourceDir = $sourceDir.Substring($zedRootWithSeparator.Length) -replace "\\", "/"
    } elseif ($sourceDir.Equals($ZedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        $relativeSourceDir = "."
    } else {
        throw "Crate source '$sourceDir' is outside Zed root '$ZedRoot'."
    }

    $destDir = Join-Path (Join-Path $RepoRoot "vendor/gpui/crates") $package.name
    [PSCustomObject]@{
        Crate = $package.name
        Source = $sourceDir
        RelativeSource = $relativeSourceDir
        Destination = $destDir
        License = if ([string]::IsNullOrWhiteSpace($package.license)) { "see crate manifest" } else { $package.license }
    }
}

if ($DryRun) {
    foreach ($row in $selectedRows) {
        Write-Host "Would copy $($row.Crate) from $($row.Source) to $($row.Destination)"
    }

    $selectedRows | Format-Table -AutoSize
    return
}

$sourceRevisionOutput = & git -C $ZedRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($sourceRevisionOutput)) {
    throw "git rev-parse HEAD failed in '$ZedRoot'."
}
$sourceRevision = $sourceRevisionOutput.Trim()

$vendorRoot = Join-Path $RepoRoot "vendor/gpui"
$vendorCrateRoot = Join-Path $vendorRoot "crates"
New-Item -ItemType Directory -Force -Path $vendorCrateRoot | Out-Null
$resolvedVendorCrateRoot = Resolve-ExistingPath $vendorCrateRoot

foreach ($row in $selectedRows) {
    $destination = [System.IO.Path]::GetFullPath($row.Destination)
    Assert-IsUnderPath -ChildPath $destination -ParentPath $resolvedVendorCrateRoot -Description "Crate destination"

    if (Test-Path -LiteralPath $destination) {
        $resolvedDestination = Resolve-ExistingPath $destination
        Assert-IsUnderPath -ChildPath $resolvedDestination -ParentPath $resolvedVendorCrateRoot -Description "Existing crate destination"
        Remove-Item -LiteralPath $resolvedDestination -Recurse -Force
    }

    Copy-Item -LiteralPath $row.Source -Destination $destination -Recurse -Force
    $resolvedCopiedDestination = Resolve-ExistingPath $destination

    $nestedTargetDirs = Get-ChildItem -LiteralPath $resolvedCopiedDestination -Recurse -Directory -Force -Filter "target"
    foreach ($targetDir in $nestedTargetDirs) {
        $resolvedTargetDir = Resolve-ExistingPath $targetDir.FullName
        Assert-IsUnderPath -ChildPath $resolvedTargetDir -ParentPath $resolvedCopiedDestination -Description "Nested target directory"
        Remove-Item -LiteralPath $resolvedTargetDir -Recurse -Force
    }
}

$readmeRows = foreach ($row in $selectedRows) {
    "| $($row.Crate) | $($row.RelativeSource) | $($row.License) |"
}

$readme = @(
    "# Vendored GPUI Closure",
    "",
    "Source: zed/",
    "",
    "Source revision: $sourceRevision",
    "",
    "| Crate | Source | License |",
    "| --- | --- | --- |"
) + $readmeRows

$readmePath = Join-Path $vendorRoot "README.md"
Set-Content -LiteralPath $readmePath -Value $readme -Encoding UTF8

$selectedRows | Format-Table -AutoSize
