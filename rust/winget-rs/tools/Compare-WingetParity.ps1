param(
    [string]$RustWinget = (Join-Path $PSScriptRoot "..\target\debug\winget.exe"),
    [string]$SystemWinget = "winget",
    [string[]]$Cases
)

$defaultCases = @(
    @{
        Name = "search-powertoys"
        Args = @("search", "PowerToys", "--count", "5")
    },
    @{
        Name = "search-tag-terminal"
        Args = @("search", "--tag", "terminal", "--count", "5")
    },
    @{
        Name = "show-powertoys"
        Args = @("show", "Microsoft.PowerToys", "--installer-type", "exe", "--architecture", "x64", "--locale", "en-US")
    },
    @{
        Name = "list-powertoys"
        Args = @("list", "Microsoft.PowerToys")
    },
    @{
        Name = "list-azcopy-upgrade"
        Args = @("list", "Microsoft.Azure.AZCopy.10", "--upgrade-available")
    }
)

if (-not (Test-Path $RustWinget)) {
    throw "Rust winget binary not found at '$RustWinget'. Build it first with 'cargo build --manifest-path rust\winget-rs\Cargo.toml -p winget-cli'."
}

function Invoke-WingetCapture {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $lines = & $Executable @Arguments 2>&1 | ForEach-Object { $_.ToString() }
    [pscustomobject]@{
        ExitCode = $LASTEXITCODE
        RawLines = @($lines)
        NormalizedLines = @(Normalize-WingetOutput -Lines $lines)
    }
}

function Normalize-WingetOutput {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string[]]$Lines
    )

    foreach ($line in $Lines) {
        $trimmed = $line.TrimEnd()
        if (-not $trimmed) {
            continue
        }
        if ($trimmed -match '^[\s\\/\-|]+$') {
            continue
        }
        if ($trimmed -like 'Failed in attempting to update the source:*') {
            continue
        }
        if ($trimmed -like 'Failed when searching source*') {
            continue
        }
        if ($trimmed -like 'warning:*REST search request failed*') {
            continue
        }
        ($trimmed -replace '\s+', ' ')
    }
}

function Select-CaseSet {
    param(
        [string[]]$RequestedCases
    )

    if (-not $RequestedCases -or $RequestedCases.Count -eq 0) {
        return $defaultCases
    }

    $selected = foreach ($requested in $RequestedCases) {
        $match = $defaultCases | Where-Object { $_.Name -eq $requested }
        if (-not $match) {
            throw "Unknown case '$requested'. Available cases: $($defaultCases.Name -join ', ')"
        }
        $match
    }

    return @($selected)
}

function Write-CaseReport {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Case,
        [Parameter(Mandatory = $true)]
        $RustResult,
        [Parameter(Mandatory = $true)]
        $SystemResult
    )

    $commandText = ($Case.Args | ForEach-Object {
            if ($_ -match '\s') {
                '"' + $_ + '"'
            } else {
                $_
            }
        }) -join ' '
    $sameOutput = ($RustResult.ExitCode -eq $SystemResult.ExitCode) -and
        (@($RustResult.NormalizedLines) -join "`n") -eq (@($SystemResult.NormalizedLines) -join "`n")

    Write-Host ("=" * 80)
    Write-Host ("CASE   : {0}" -f $Case.Name)
    Write-Host ("COMMAND: winget {0}" -f $commandText)
    Write-Host ("STATUS : {0}" -f ($(if ($sameOutput) { "MATCH" } else { "DIFF" })))
    Write-Host ("RUST EXIT   : {0}" -f $RustResult.ExitCode)
    Write-Host ("SYSTEM EXIT : {0}" -f $SystemResult.ExitCode)
    Write-Host "--- RUST ---"
    if ($RustResult.NormalizedLines.Count -eq 0) {
        Write-Host "<no output>"
    } else {
        $RustResult.NormalizedLines | ForEach-Object { Write-Host $_ }
    }
    Write-Host "--- SYSTEM ---"
    if ($SystemResult.NormalizedLines.Count -eq 0) {
        Write-Host "<no output>"
    } else {
        $SystemResult.NormalizedLines | ForEach-Object { Write-Host $_ }
    }

    if (-not $sameOutput) {
        Write-Host "--- DIFF ---"
        Compare-Object -ReferenceObject $RustResult.NormalizedLines -DifferenceObject $SystemResult.NormalizedLines -SyncWindow 0 |
            ForEach-Object {
                Write-Host ("{0} {1}" -f $_.SideIndicator, $_.InputObject)
            }
    }
}

$caseSet = Select-CaseSet -RequestedCases $Cases
foreach ($case in $caseSet) {
    $rustResult = Invoke-WingetCapture -Executable $RustWinget -Arguments $case.Args
    $systemResult = Invoke-WingetCapture -Executable $SystemWinget -Arguments $case.Args
    Write-CaseReport -Case $case -RustResult $rustResult -SystemResult $systemResult
}
