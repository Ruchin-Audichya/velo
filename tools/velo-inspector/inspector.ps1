<#
.SYNOPSIS
  Look at and drive a running Velo window, for debugging the interface.

.DESCRIPTION
  One action per call, JSON on stdout. Used by velo_inspector.py (the MCP
  server) and usable on its own from a terminal.

  Actions:
    screenshot  -Out <png> [-Scale 0.6]   Capture Velo's visible window frame.
    tree        [-Depth 12]               UI Automation tree: names, ids, rects.
    click       -X <n> -Y <n>             Click at window-relative pixels.
    rightclick  -X <n> -Y <n>             Right-click (opens context menus).
    element     -Id <automationId> | -Name <text>   Click an element by UIA.
    keys        -Text <SendKeys string>   Send keystrokes (e.g. ^, for Ctrl+,).
    scroll      -X <n> -Y <n> -Delta <n>  Wheel at window-relative pixels.
    status                                Window, memory, and connection state.

  Coordinates are physical pixels relative to the visible window frame — the
  same space as an unscaled screenshot. With -Scale, divide by the scale.
  Only Velo's own window is ever captured or clicked.
#>
param(
    [Parameter(Mandatory = $true)][ValidateSet('screenshot', 'tree', 'click', 'rightclick', 'element', 'keys', 'scroll', 'status')][string]$Action,
    [string]$Out,
    [double]$Scale = 1.0,
    [int]$Depth = 12,
    [int]$X, [int]$Y, [int]$Delta = -360,
    [string]$Id, [string]$Name, [string]$Text
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('VeloWin' -as [type])) {
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class VeloWin {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, UIntPtr e);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out R r, int s);
}
'@
}
[void][VeloWin]::SetProcessDPIAware()

function Get-VeloWindow {
    $p = Get-Process velo -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1
    if (!$p) { throw 'Velo is not running (no velo window).' }
    if ([VeloWin]::IsIconic($p.MainWindowHandle)) { [void][VeloWin]::ShowWindow($p.MainWindowHandle, 9) }
    $r = New-Object VeloWin+R
    [void][VeloWin]::DwmGetWindowAttribute($p.MainWindowHandle, 9, [ref]$r, 16)
    [pscustomobject]@{ Process = $p; Handle = $p.MainWindowHandle; Rect = $r }
}

function Focus-Velo($w) {
    [void][VeloWin]::SetForegroundWindow($w.Handle)
    $shell = New-Object -ComObject WScript.Shell
    [void]$shell.AppActivate($w.Process.Id)
    Start-Sleep -Milliseconds 250
}

function Click-At($w, [int]$cx, [int]$cy, [switch]$Right) {
    Focus-Velo $w
    [void][VeloWin]::SetCursorPos($w.Rect.L + $cx, $w.Rect.T + $cy)
    # Arrive like a hand would: a move event, then a beat. gpui answers
    # Windows' hit test for the title-bar buttons from the last position it
    # was told about, so a pointer that teleports and clicks at once is read
    # at its *previous* spot — a maximise click came out as Close.
    [VeloWin]::mouse_event(1, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    # Left: DOWN 0x2 / UP 0x4. Right: DOWN 0x8 / UP 0x10.
    $down = if ($Right) { 8 } else { 2 }
    [VeloWin]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero)
    [VeloWin]::mouse_event($down * 2, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

$w = Get-VeloWindow
$width = $w.Rect.Rt - $w.Rect.L; $height = $w.Rect.B - $w.Rect.T

switch ($Action) {
    'screenshot' {
        if (!$Out) { throw '-Out is required.' }
        Focus-Velo $w
        $bmp = New-Object System.Drawing.Bitmap $width, $height
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($w.Rect.L, $w.Rect.T, 0, 0, $bmp.Size); $g.Dispose()
        if ($Scale -ne 1.0) {
            $scaled = New-Object System.Drawing.Bitmap $bmp, ([int]($width * $Scale)), ([int]($height * $Scale))
            $bmp.Dispose(); $bmp = $scaled
        }
        $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
        @{ path = $Out; width = $width; height = $height; scale = $Scale } | ConvertTo-Json -Compress
    }
    'tree' {
        Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($w.Handle)
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        $items = New-Object System.Collections.Generic.List[object]
        function Walk($el, $level) {
            if ($level -gt $Depth -or $items.Count -gt 1500) { return }
            $c = $el.Current; $b = $c.BoundingRectangle
            if (!$b.IsEmpty -and ($c.Name -or $c.AutomationId)) {
                $items.Add([ordered]@{
                    depth = $level; type = ($c.ControlType.ProgrammaticName -replace '^ControlType\.', '')
                    name = $c.Name; id = $c.AutomationId
                    x = [int]($b.Left - $w.Rect.L); y = [int]($b.Top - $w.Rect.T); w = [int]$b.Width; h = [int]$b.Height
                })
            }
            $child = $walker.GetFirstChild($el)
            while ($child) { Walk $child ($level + 1); $child = $walker.GetNextSibling($child) }
        }
        Walk $root 0
        $items | ConvertTo-Json -Depth 3 -Compress
    }
    'click' { Click-At $w $X $Y; @{ clicked = @($X, $Y) } | ConvertTo-Json -Compress }
    'rightclick' { Click-At $w $X $Y -Right; @{ right_clicked = @($X, $Y) } | ConvertTo-Json -Compress }
    'element' {
        Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($w.Handle)
        $prop = if ($Id) { [System.Windows.Automation.AutomationElement]::AutomationIdProperty } else { [System.Windows.Automation.AutomationElement]::NameProperty }
        $value = if ($Id) { $Id } else { $Name }
        $found = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition $prop, $value))
        if (!$found) { throw "No element with $(if ($Id) {'id'} else {'name'}) '$value'." }
        $pattern = $null
        if ($found.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
            Focus-Velo $w; $pattern.Invoke(); Start-Sleep -Milliseconds 500
        } else {
            $b = $found.Current.BoundingRectangle
            Click-At $w ([int]($b.Left + $b.Width / 2 - $w.Rect.L)) ([int]($b.Top + $b.Height / 2 - $w.Rect.T))
        }
        @{ invoked = $value } | ConvertTo-Json -Compress
    }
    'keys' {
        if (!$Text) { throw '-Text is required.' }
        Focus-Velo $w
        (New-Object -ComObject WScript.Shell).SendKeys($Text); Start-Sleep -Milliseconds 600
        @{ sent = $Text } | ConvertTo-Json -Compress
    }
    'scroll' {
        Focus-Velo $w
        [void][VeloWin]::SetCursorPos($w.Rect.L + $X, $w.Rect.T + $Y)
        [VeloWin]::mouse_event(0x0800, 0, 0, $Delta, [UIntPtr]::Zero); Start-Sleep -Milliseconds 400
        @{ scrolled = $Delta } | ConvertTo-Json -Compress
    }
    'status' {
        $daemon = Get-Process velod -ErrorAction SilentlyContinue | Select-Object -First 1
        $cli = Join-Path (Split-Path $w.Process.Path) 'velo-cli.exe'
        $state = try { (& $cli --read-only --json status 2>$null | ConvertFrom-Json).data } catch { $null }
        [ordered]@{
            title = $w.Process.MainWindowTitle; width = $width; height = $height
            window_mb = [int]($w.Process.WorkingSet64 / 1MB)
            daemon_mb = if ($daemon) { [int]($daemon.WorkingSet64 / 1MB) } else { $null }
            session = $state
        } | ConvertTo-Json -Depth 4 -Compress
    }
}
