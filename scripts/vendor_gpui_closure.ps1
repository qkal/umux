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

function Get-RelativePathCompat {
    param(
        [Parameter(Mandatory = $true)][string]$FromDirectory,
        [Parameter(Mandatory = $true)][string]$ToPath
    )

    $fromFull = [System.IO.Path]::GetFullPath($FromDirectory)
    $toFull = [System.IO.Path]::GetFullPath($ToPath)
    if (-not $fromFull.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $fromFull = "$fromFull$([System.IO.Path]::DirectorySeparatorChar)"
    }

    $fromUri = [System.Uri]::new($fromFull)
    $toUri = [System.Uri]::new($toFull)
    if (-not $fromUri.Scheme.Equals($toUri.Scheme, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Cannot compute relative path between '$FromDirectory' and '$ToPath'."
    }

    $relativePath = [System.Uri]::UnescapeDataString($fromUri.MakeRelativeUri($toUri).ToString())
    if ($fromUri.Scheme.Equals("file", [System.StringComparison]::OrdinalIgnoreCase)) {
        $relativePath = $relativePath.Replace("/", [System.IO.Path]::DirectorySeparatorChar)
    }

    return $relativePath
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

function Get-PackageLicense {
    param(
        [Parameter(Mandatory = $true)]$Package,
        [Parameter(Mandatory = $true)][string]$SourceDir
    )

    if (-not [string]::IsNullOrWhiteSpace($Package.license)) {
        return $Package.license
    }

    $licenseLabelsByFile = @{
        "LICENSE-AGPL" = "AGPL-3.0-or-later"
        "LICENSE-APACHE" = "Apache-2.0"
        "LICENSE-GPL" = "GPL-3.0-or-later"
    }

    $inferredLicenses = foreach ($licenseFile in ($licenseLabelsByFile.Keys | Sort-Object)) {
        $licensePath = Join-Path $SourceDir $licenseFile
        if (Test-Path -LiteralPath $licensePath -PathType Leaf) {
            $licenseLabelsByFile[$licenseFile]
        }
    }

    if ($inferredLicenses.Count -gt 0) {
        return ($inferredLicenses -join " OR ")
    }

    return "see crate manifest"
}

function Copy-RootLicenseFiles {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )

    $licenseFiles = @("LICENSE-APACHE", "LICENSE-GPL", "LICENSE-AGPL")
    New-Item -ItemType Directory -Force -Path $DestinationRoot | Out-Null

    foreach ($licenseFile in $licenseFiles) {
        $sourceLicense = Join-Path $SourceRoot $licenseFile
        if (Test-Path -LiteralPath $sourceLicense -PathType Leaf) {
            Copy-Item -LiteralPath $sourceLicense -Destination (Join-Path $DestinationRoot $licenseFile) -Force
        }
    }
}

function Get-RequiredAssetRows {
    param(
        [Parameter(Mandatory = $true)][string]$SourceRoot,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )

    $assetRelativePaths = @(
        "assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf",
        "assets/fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf",
        "assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf",
        "assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf",
        "assets/fonts/ibm-plex-sans/license.txt",
        "assets/fonts/lilex/Lilex-Regular.ttf",
        "assets/fonts/lilex/Lilex-Bold.ttf",
        "assets/fonts/lilex/Lilex-Italic.ttf",
        "assets/fonts/lilex/Lilex-BoldItalic.ttf",
        "assets/fonts/lilex/OFL.txt"
    )

    foreach ($assetRelativePath in $assetRelativePaths) {
        [PSCustomObject]@{
            RelativePath = $assetRelativePath
            Source = Join-Path $SourceRoot $assetRelativePath
            Destination = Join-Path $DestinationRoot $assetRelativePath
        }
    }
}

function Copy-RequiredAssets {
    param(
        [Parameter(Mandatory = $true)]$AssetRows,
        [Parameter(Mandatory = $true)][string]$VendorRoot
    )

    foreach ($assetRow in $AssetRows) {
        if (-not (Test-Path -LiteralPath $assetRow.Source -PathType Leaf)) {
            throw "Required GPUI asset '$($assetRow.RelativePath)' was not found at '$($assetRow.Source)'."
        }

        $destination = [System.IO.Path]::GetFullPath($assetRow.Destination)
        Assert-IsUnderPath -ChildPath $destination -ParentPath $VendorRoot -Description "Asset destination"

        $destinationParent = Split-Path -Parent $destination
        New-Item -ItemType Directory -Force -Path $destinationParent | Out-Null
        Copy-Item -LiteralPath $assetRow.Source -Destination $destination -Force
    }
}

function Repair-GpuiMacrosDevDependency {
    param(
        [Parameter(Mandatory = $true)][string]$VendorRoot
    )

    $manifestPath = Join-Path $VendorRoot "crates/gpui_macros/Cargo.toml"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "gpui_macros manifest was not found at '$manifestPath'."
    }

    $original = Get-Content -LiteralPath $manifestPath -Raw
    $expected = 'gpui = { workspace = true, features = ["inspector"] }'
    $replacement = 'gpui = { workspace = true, features = ["inspector", "test-support"] }'

    if ($original.Contains($replacement)) {
        return
    }

    if (-not $original.Contains($expected)) {
        throw "gpui_macros dev-dependency layout changed; expected to find '$expected' in '$manifestPath'."
    }

    $updated = $original.Replace($expected, $replacement)
    Set-Content -LiteralPath $manifestPath -Value $updated -Encoding UTF8
}

function Assert-LicensePointersResolve {
    param(
        [Parameter(Mandatory = $true)][string]$CrateRoot
    )

    if (-not (Test-Path -LiteralPath $CrateRoot -PathType Container)) {
        return
    }

    $licenseFiles = Get-ChildItem -LiteralPath $CrateRoot -Recurse -Force -File -Filter "LICENSE-*"
    foreach ($licenseFile in $licenseFiles) {
        $content = Get-Content -LiteralPath $licenseFile.FullName -Raw
        if ([string]::IsNullOrWhiteSpace($content)) {
            continue
        }

        $pointer = $content.TrimStart()
        if (-not ($pointer.StartsWith("../", [System.StringComparison]::Ordinal) -or $pointer.StartsWith("..\", [System.StringComparison]::Ordinal))) {
            continue
        }

        $pointerTarget = ($pointer -split "\r?\n", 2)[0].Trim()
        $resolvedTarget = [System.IO.Path]::GetFullPath((Join-Path $licenseFile.DirectoryName $pointerTarget))
        if (-not (Test-Path -LiteralPath $resolvedTarget -PathType Leaf)) {
            throw "License pointer '$($licenseFile.FullName)' points to missing target '$resolvedTarget'."
        }
    }
}

function Repair-LicensePointersToVendorRoot {
    param(
        [Parameter(Mandatory = $true)][string]$CrateRoot,
        [Parameter(Mandatory = $true)][string]$VendorRoot
    )

    if (-not (Test-Path -LiteralPath $CrateRoot -PathType Container)) {
        return
    }

    $licenseFiles = Get-ChildItem -LiteralPath $CrateRoot -Recurse -Force -File -Filter "LICENSE-*"
    foreach ($licenseFile in $licenseFiles) {
        $content = Get-Content -LiteralPath $licenseFile.FullName -Raw
        if ([string]::IsNullOrWhiteSpace($content)) {
            continue
        }

        $pointer = $content.TrimStart()
        if (-not ($pointer.StartsWith("../", [System.StringComparison]::Ordinal) -or $pointer.StartsWith("..\", [System.StringComparison]::Ordinal))) {
            continue
        }

        $pointerTarget = ($pointer -split "\r?\n", 2)[0].Trim()
        $resolvedTarget = [System.IO.Path]::GetFullPath((Join-Path $licenseFile.DirectoryName $pointerTarget))
        if (Test-Path -LiteralPath $resolvedTarget -PathType Leaf) {
            continue
        }

        $vendorRootTarget = Join-Path $VendorRoot (Split-Path -Leaf $pointerTarget)
        if (-not (Test-Path -LiteralPath $vendorRootTarget -PathType Leaf)) {
            continue
        }

        $relativeTarget = (Get-RelativePathCompat -FromDirectory $licenseFile.DirectoryName -ToPath $vendorRootTarget) -replace "\\", "/"
        Set-Content -LiteralPath $licenseFile.FullName -Value $relativeTarget -NoNewline -Encoding UTF8
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
        License = Get-PackageLicense -Package $package -SourceDir $sourceDir
    }
}

$assetRows = Get-RequiredAssetRows -SourceRoot $ZedRoot -DestinationRoot (Join-Path $RepoRoot "vendor/gpui")

if ($DryRun) {
    foreach ($row in $selectedRows) {
        Write-Host "Would copy $($row.Crate) from $($row.Source) to $($row.Destination)"
    }

    foreach ($assetRow in $assetRows) {
        if (Test-Path -LiteralPath $assetRow.Source -PathType Leaf) {
            Write-Host "Would copy asset $($assetRow.RelativePath) from $($assetRow.Source) to $($assetRow.Destination)"
        } else {
            Write-Host "Missing required asset $($assetRow.RelativePath) at $($assetRow.Source)"
        }
    }

    Write-Host "Would enable gpui test-support for gpui_macros doctests in vendor/gpui/crates/gpui_macros/Cargo.toml"

    $selectedRows | Format-Table -AutoSize
    $assetRows | Select-Object RelativePath, Source, Destination | Format-Table -AutoSize
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

Copy-RootLicenseFiles -SourceRoot $ZedRoot -DestinationRoot $vendorRoot
Copy-RequiredAssets -AssetRows $assetRows -VendorRoot $vendorRoot

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

Repair-GpuiMacrosDevDependency -VendorRoot $vendorRoot
Repair-LicensePointersToVendorRoot -CrateRoot $resolvedVendorCrateRoot -VendorRoot $vendorRoot
Assert-LicensePointersResolve -CrateRoot $resolvedVendorCrateRoot

$readmeRows = foreach ($row in $selectedRows) {
    "| $($row.Crate) | $($row.RelativeSource) | $($row.License) |"
}

$assetReadmeRows = foreach ($assetRow in $assetRows) {
    $license = switch -Wildcard ($assetRow.RelativePath) {
        "assets/fonts/lilex/*" { "SIL Open Font License 1.1" }
        "assets/fonts/ibm-plex-sans/*" { "SIL Open Font License 1.1" }
        default { "see source asset license" }
    }

    "| $($assetRow.RelativePath) | $license |"
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
) + $readmeRows + @(
    "",
    "## Non-Crate Assets",
    "",
    "GPUI tests, examples, and web platform code embed the following font assets from `zed/assets/fonts`. Keep the adjacent font license files with the font files when regenerating the vendor closure.",
    "",
    "| Asset | License |",
    "| --- | --- |"
) + $assetReadmeRows + @(
    "",
    "## Workspace Dependency Resolutions",
    "",
    "- `windows`: umux previously used `0.62.2`; GPUI/Zed uses `0.61` with the full Zed feature set. The workspace dependency now follows Zed. `cargo check -p umux-win32` passed without adding a crate-local override; `webview2-com` still brings `windows 0.62.2` transitively where it needs it.",
    "- GPUI-facing shared dependencies now follow the Zed workspace specs where they differed from umux: `anyhow = 1.0.86`, `serde = 1.0.221` with `derive` and `rc`, `serde_json = 1.0.144` with `preserve_order` and `raw_value`, `thiserror = 2.0.12`, and `tracing = 0.1.40`.",
    "- `wgpu` follows Zed's fork and is pinned to the Zed-tested `v29` revision `a466bc382ea747f8e1ac810efdb6dcd49a514575` instead of floating on the branch.",
    "- `reqwest_client` and `http_client_tls` are vendored support crates because GPUI dev-dependencies inherit them from the workspace even though the smoke app does not compile those dev targets.",
    "- `gpui_macros` keeps doctests enabled like Zed, but its vendored dev-dependency enables GPUI's `test-support` feature so `cargo test --workspace` has the proptest helpers exported during doctest compilation."
)

$readmePath = Join-Path $vendorRoot "README.md"
Set-Content -LiteralPath $readmePath -Value $readme -Encoding UTF8

$selectedRows | Format-Table -AutoSize
$assetRows | Select-Object RelativePath, Source, Destination | Format-Table -AutoSize
