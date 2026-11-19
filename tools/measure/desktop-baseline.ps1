<#
.SYNOPSIS
  Measure the installed Velo: cold start, idle memory, binary sizes.

.DESCRIPTION
  The before/after yardstick for performance work. Run it against the
  installed build (per-user install under %LOCALAPPDATA%\Programs\Velo) on the
  same machine each time, with nothing else heavy running:

    pwsh tools/measure/desktop-baseline.ps1 -Label baseline

  It appends one row per run to docs/measurements/desktop-baseline.csv and
  prints a summary. What it measures:

  - window_ms: starting the window, with the background service already
    running, until Windows reports its main window. A proxy for first frame:
    the window is created when gpui has a surface to draw.
  - service_ms: starting the background service until its pipe accepts a
    connection.
  - connected_ms: starting the background service until it reports the
    WhatsApp session connected. Network-bound; read it as a range.
  - window_mb / service_mb: private memory 20 s after the window is shown,
    idle on the chat list.
  - Sizes of the shipped executables and the last-built installer.

  Restarting the service reconnects your WhatsApp session, which is what it
  does after every sign-in; nothing is lost.
#>
param(
    [string]$Label = 'run',
    [int]$Runs = 3
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$install = Join-Path $env:LOCALAPPDATA 'Programs\Velo'
$window = Join-Path $install 'oxidezap.exe'
$service = Join-Path $install 'oxidezapd.exe'
if (!(Test-Path $window)) { throw "Velo is not installed at $install" }

function Stop-Window {
    Get-Process oxidezap -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 800
}

function Pipe-Name {
    $suffix = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value -replace '[^0-9A-Za-z]', '_'
    "oxidezap-$suffix"
}

# One request to the background service over its wire protocol: `get_status`
# or `shutdown`. Returns the session state for a status request.
function Invoke-Service([string]$action) {
    $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', (Pipe-Name), [IO.Pipes.PipeDirection]::InOut)
    try {
        $pipe.Connect(2000)
        $utf8 = [Text.UTF8Encoding]::new($false)
        $writer = [IO.StreamWriter]::new($pipe, $utf8, 4096, $true)
        $reader = [IO.StreamReader]::new($pipe, $utf8, $false, 4096, $true)
        $writer.NewLine = "`n"; $writer.AutoFlush = $true
        $read = {
            $task = $reader.ReadLineAsync()
            if (!$task.Wait(5000) -or !$task.Result) { throw 'no answer' }
            $task.Result | ConvertFrom-Json -Depth 32
        }
        $readOnly = ($action -eq 'get_status').ToString().ToLowerInvariant()
        $writer.WriteLine('{"id":1,"action":"hello","params":{"protocol":1,"client_name":"measure","read_only":' + $readOnly + ',"session_events":false}}')
        do { $answer = & $read } while ($answer.id -ne 1)
        $writer.WriteLine('{"id":2,"action":"' + $action + '"}')
        do { $answer = & $read } while ($answer.id -ne 2)
        return $answer.data.state
    } finally { $pipe.Dispose() }
}

function Wait-Pipe([int]$timeoutMs) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.ElapsedMilliseconds -lt $timeoutMs) {
        $pipe = [IO.Pipes.NamedPipeClientStream]::new('.', (Pipe-Name), [IO.Pipes.PipeDirection]::InOut)
        try { $pipe.Connect(50); $pipe.Dispose(); return $true } catch { $pipe.Dispose() }
    }
    return $false
}

function Measure-Window {
    Stop-Window
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process $window -PassThru
    while ($watch.ElapsedMilliseconds -lt 30000) {
        $process.Refresh()
        if ($process.MainWindowHandle -ne 0) { break }
        Start-Sleep -Milliseconds 10
    }
    return $watch.ElapsedMilliseconds
}

function Measure-Service {
    Stop-Window
    try { Invoke-Service 'shutdown' | Out-Null } catch {}
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Process oxidezapd -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Start-Process $service | Out-Null
    $ready = if (Wait-Pipe 30000) { $watch.ElapsedMilliseconds } else { -1 }
    $connected = -1
    while ($watch.ElapsedMilliseconds -lt 60000) {
        try { if ((Invoke-Service 'get_status') -eq 'connected') { $connected = $watch.ElapsedMilliseconds; break } } catch {}
        Start-Sleep -Milliseconds 250
    }
    return @($ready, $connected)
}

function Private-MB([string]$name) {
    $p = Get-Process $name -ErrorAction SilentlyContinue | Select-Object -First 1
    if (!$p) { return -1 }
    return [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
}

$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1)
$gpu = (Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name) -join ' + '
$ram = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
$os = (Get-CimInstance Win32_OperatingSystem)
$version = (Get-Item $window).VersionInfo.ProductVersion
$machine = "$($cpu.Name.Trim()) ($($cpu.NumberOfLogicalProcessors) threads), $ram GB RAM, $gpu, $($os.Caption) $($os.BuildNumber)"
Write-Host "Velo $version on $machine"

$csv = Join-Path $root 'docs\measurements\desktop-baseline.csv'
if (!(Test-Path $csv)) {
    'label,version,run,window_ms,service_ms,connected_ms,window_mb,service_mb,window_exe_mb,service_exe_mb,installer_mb,machine' | Set-Content -Encoding utf8 $csv
}
$size = { param($path) if (Test-Path $path) { [math]::Round((Get-Item $path).Length / 1MB, 1) } else { -1 } }
$installer = Get-ChildItem (Join-Path $root 'dist') -Filter 'Velo-Setup-*.exe' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1

for ($run = 1; $run -le $Runs; $run++) {
    $serviceTimes = Measure-Service
    $windowMs = Measure-Window
    Start-Sleep -Seconds 20
    $row = @(
        $Label, $version, $run, $windowMs, $serviceTimes[0], $serviceTimes[1],
        (Private-MB oxidezap), (Private-MB oxidezapd),
        (& $size $window), (& $size $service), (& $size $installer.FullName),
        "`"$machine`""
    ) -join ','
    Add-Content -Encoding utf8 $csv $row
    Write-Host $row
}
