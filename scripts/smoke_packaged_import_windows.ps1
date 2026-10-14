# Imports a pack through the installed Windows app's real UI with UI Automation (release
# WebView2 builds disable the DevTools channel msedgedriver needs).
# Usage: powershell -File scripts/smoke_packaged_import_windows.ps1 -App <exe> -Pack <path>
param(
    [Parameter(Mandatory = $true)][string]$App,
    [Parameter(Mandatory = $true)][string]$Pack
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
$Automation = [System.Windows.Automation.AutomationElement]
$Tree = [System.Windows.Automation.TreeScope]

function Wait-Until([string]$Description, [int]$Seconds, [scriptblock]$Probe) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $found = & $Probe
        if ($found) { return $found }
        Start-Sleep -Seconds 1
    }
    throw "timed out after ${Seconds}s waiting for $Description`n$(Get-VisibleText)"
}

# WebView2 content belongs to msedgewebview2, not the app process, so searches start at the
# desktop root rather than at the app's main window handle.
$Root = $Automation::RootElement

# Only the app and its WebView2 processes count; the desktop root also holds the taskbar and
# tray, whose labels ("No audio device is installed") would otherwise match.
function Get-AppProcessIds {
    return @($script:AppProcess.Id) + @(Get-Process msedgewebview2 -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
}

function Get-AppElements($Condition) {
    $ids = Get-AppProcessIds
    return $Root.FindAll($Tree::Descendants, $Condition) | Where-Object { $_.Current.ProcessId -in $ids }
}

function Get-VisibleText {
    $windows = $Root.FindAll($Tree::Children, [System.Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { "window '$($_.Current.Name)' class=$($_.Current.ClassName) pid=$($_.Current.ProcessId)" }
    $names = Get-AppElements ([System.Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { "$($_.Current.ControlType.ProgrammaticName) '$($_.Current.Name)'" } |
        Select-Object -First 200
    return (@($windows) + @($names)) -join "`n"
}

function Find-Named([string]$Name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition($Automation::NameProperty, $Name)
    foreach ($element in Get-AppElements $condition) {
        if ($element.Current.IsEnabled) { return $element }
    }
    return $null
}

# Labels repeat (the sidebar link and the page heading are both "Packs"), so only an element
# that can actually be invoked counts.
function Find-Invokable([string]$Name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition($Automation::NameProperty, $Name)
    foreach ($element in Get-AppElements $condition) {
        $pattern = $null
        if ($element.Current.IsEnabled -and
            $element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
            return , $pattern
        }
    }
    return $null
}

function Invoke-Named([string]$Name) {
    $pattern = Wait-Until $Name 60 { Find-Invokable $Name }
    $pattern.Invoke()
}

$process = Start-Process -FilePath $App -PassThru
$script:AppProcess = $process
try {
    $null = Wait-Until 'the app window' 60 {
        $process.Refresh()
        if ($process.HasExited) { throw "the app exited with code $($process.ExitCode)" }
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $process.MainWindowHandle }
    }

    Invoke-Named 'Packs'
    $field = Wait-Until 'the pack path field' 60 { Find-Named 'Local pack path' }
    $field.SetFocus()
    # SendKeys treats + ^ % ~ ( ) { } [ ] as commands; brace every character so the path is literal.
    $literal = -join ($Pack.ToCharArray() | ForEach-Object { if ('+^%~(){}[]'.Contains($_)) { "{$_}" } else { $_ } })
    [System.Windows.Forms.SendKeys]::SendWait($literal)
    Invoke-Named 'Review pack'
    Invoke-Named 'Install pack'

    $heading = Wait-Until 'the install result' 300 {
        Get-AppElements ([System.Windows.Automation.Condition]::TrueCondition) |
            ForEach-Object { $_.Current.Name } |
            Where-Object { $_ -match ' (is (already )?installed|was not installed)$' } |
            Select-Object -First 1
    }
    Write-Output $heading
    if ($heading -match ' was not installed$') {
        throw "pack import failed`n$(Get-VisibleText)"
    }
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Get-Process msedgewebview2 -ErrorAction SilentlyContinue | Stop-Process -Force
}
