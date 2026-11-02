param([Parameter(Mandatory=$true)][string]$Config)
$ErrorActionPreference = 'Stop'
$c = Get-Content -LiteralPath $Config -Raw | ConvertFrom-Json
. (Join-Path $PSScriptRoot 'graphics.ps1')
[BenchGraphics]::PhysicalCoordinates()
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class BenchWindow {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int L,T,R,B; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref Point p);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint flag);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x,int y,int w,int height,uint flags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint msg,IntPtr w,IntPtr l);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h,int attr,out int value,int size);
  [DllImport("kernel32.dll")] public static extern bool GetSystemTimes(out long idle,out long kernel,out long user);
  [DllImport("kernel32.dll")] public static extern bool QueryPerformanceCounter(out long value);
  [DllImport("kernel32.dll")] public static extern bool QueryPerformanceFrequency(out long value);
  public static long Counter() { long n; QueryPerformanceCounter(out n); return n; }
  public static double QpcMs() { long f; QueryPerformanceFrequency(out f); return 1000.0*Counter()/f; }
  public static long[] Cpu() { long i,k,u; GetSystemTimes(out i,out k,out u); return new long[]{i,k+u}; }
  public static bool Occluded(IntPtr target) {
    Rect r; GetWindowRect(target,out r);
    for (IntPtr h=GetWindow(target,3); h!=IntPtr.Zero; h=GetWindow(h,3)) {
      int cloaked; DwmGetWindowAttribute(h,14,out cloaked,4);
      if (!IsWindowVisible(h)||IsIconic(h)||cloaked!=0) continue;
      Rect a; GetWindowRect(h,out a);
      if (Math.Min(r.R,a.R)>Math.Max(r.L,a.L)&&Math.Min(r.B,a.B)>Math.Max(r.T,a.T)) return true;
    }
    return false;
  }
}
'@
function Find-Window {
  $p = Get-Process -Id $c.clientPid -ErrorAction SilentlyContinue
  if ($p) { $p.Refresh(); return $p.MainWindowHandle }
  return [IntPtr]::Zero
}
function Key([IntPtr]$h, [int]$key) {
  [BenchWindow]::PostMessage($h,0x100,[IntPtr]$key,[IntPtr]0) | Out-Null
  Start-Sleep -Milliseconds 40
  [BenchWindow]::PostMessage($h,0x101,[IntPtr]$key,[IntPtr]0) | Out-Null
}
function Click([IntPtr]$h, [int]$x, [int]$y) {
  $xy = [IntPtr](($y -shl 16) -bor ($x -band 65535))
  [BenchWindow]::PostMessage($h,0x200,[IntPtr]0,$xy) | Out-Null
  [BenchWindow]::PostMessage($h,0x201,[IntPtr]1,$xy) | Out-Null
  Start-Sleep -Milliseconds 80
  [BenchWindow]::PostMessage($h,0x202,[IntPtr]0,$xy) | Out-Null
}
if ($c.mode -eq 'machine') {
  $adapters = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate)
  $screens = @([Windows.Forms.Screen]::AllScreens | ForEach-Object { @{ display=$_.DeviceName; bounds=@($_.Bounds.X,$_.Bounds.Y,$_.Bounds.Width,$_.Bounds.Height); refreshHz=[BenchGraphics]::Refresh($_.DeviceName) } })
  @{ adapters=$adapters; dxgiAdapters=@([BenchGraphics]::Adapters()); screens=$screens; logicalProcessors=[Environment]::ProcessorCount; os=[Environment]::OSVersion.VersionString } | ConvertTo-Json -Depth 5 -Compress
  exit
}
$deadline = [DateTime]::UtcNow.AddSeconds(20)
do { $hwnd = Find-Window; if ($hwnd -ne [IntPtr]::Zero) { break }; Start-Sleep -Milliseconds 100 } while ([DateTime]::UtcNow -lt $deadline)
if ($hwnd -eq [IntPtr]::Zero) { throw 'No client window within 20 seconds' }
if ($c.mode -eq 'setup') {
  $r = New-Object BenchWindow+Rect
  [BenchWindow]::GetClientRect($hwnd,[ref]$r) | Out-Null
  $outer = New-Object BenchWindow+Rect
  [BenchWindow]::GetWindowRect($hwnd,[ref]$outer) | Out-Null
  # Resize without activating or moving another window. All dimensions physical.
  [BenchWindow]::SetWindowPos($hwnd,[IntPtr]0,$c.x,$c.y,$c.width+$outer.R-$outer.L-$r.R,$c.height+$outer.B-$outer.T-$r.B,0x14) | Out-Null
  Start-Sleep -Milliseconds 800
  if ($c.client -eq 'ours') {
    Key $hwnd 13
    Start-Sleep -Seconds 3
    if ($c.scenario -eq 'battledome') {
      # Directed WM_MOUSEWHEEL; no system cursor, SendInput or foreground call.
      $pt = New-Object BenchWindow+Point
      $pt.X = [int]($c.width/2); $pt.Y = [int]($c.height/2)
      [BenchWindow]::ClientToScreen($hwnd,[ref]$pt) | Out-Null
      [BenchWindow]::PostMessage($hwnd,0x20A,[IntPtr](-12000 -shl 16),[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 65535))) | Out-Null
    }
  }
  exit
}
$prevCpu = [BenchWindow]::Cpu()
$hostProcess = Get-Process -Id $c.hostPid
$prevHost = $hostProcess.TotalProcessorTime.TotalSeconds
$prevAt = [Diagnostics.Stopwatch]::GetTimestamp()
$end = [DateTime]::UtcNow.AddSeconds($c.seconds)
$buildCommands = $false
$nextBuildCheck = [DateTime]::MinValue
$gpuEngines = @()
$gpuError = $null
$gpuCounters = @()
try {
  $category = New-Object Diagnostics.PerformanceCounterCategory('GPU Engine')
  $gpuCounters = @($category.GetInstanceNames() | Where-Object { $_ -match ('^pid_' + $c.gpuPid + '_') } | ForEach-Object {
    $counter = New-Object Diagnostics.PerformanceCounter('GPU Engine','Utilization Percentage',$_)
    $null = $counter.NextValue()
    $counter
  })
} catch { $gpuError = $_.Exception.Message }
while ([DateTime]::UtcNow -lt $end) {
  $r = New-Object BenchWindow+Rect
  [BenchWindow]::GetClientRect($hwnd,[ref]$r) | Out-Null
  $hostProcess.Refresh()
  $now = [Diagnostics.Stopwatch]::GetTimestamp(); $hostCpu = $hostProcess.TotalProcessorTime.TotalSeconds
  $elapsed = ($now-$prevAt)/[Diagnostics.Stopwatch]::Frequency
  $cpu = [BenchWindow]::Cpu(); $total = $cpu[1]-$prevCpu[1]
  $load = if ($total -gt 0) { 100*(1-($cpu[0]-$prevCpu[0])/$total) } else { 0 }
  $screen = [Windows.Forms.Screen]::FromHandle($hwnd)
  $build = @(Get-Process -Name cargo,rustc,cl,link,gcc,cc1,cc1plus,make,ninja,cmake,'wasm-bindgen' -ErrorAction SilentlyContinue).Count -gt 0
  if ([DateTime]::UtcNow -ge $nextBuildCheck) {
    $buildCommands = @(Get-CimInstance Win32_Process -Filter "Name='node.exe'" | Where-Object { $_.CommandLine -match '(--test|check\.mjs|build-web\.mjs|npm.*(test|check))' }).Count -gt 0
    $nextBuildCheck = [DateTime]::UtcNow.AddSeconds(1)
  }
  $build = $build -or $buildCommands
  try { $gpuEngines = @($gpuCounters | ForEach-Object { @{instance=$_.InstanceName; utilization=$_.NextValue()} }) }
  catch { $gpuError = $_.Exception.Message }
  @{ qpcMs=[BenchWindow]::QpcMs(); foreground=([BenchWindow]::GetForegroundWindow() -eq $hwnd);
     occluded=[BenchWindow]::Occluded($hwnd); minimized=[BenchWindow]::IsIconic($hwnd); visible=[BenchWindow]::IsWindowVisible($hwnd);
     width=$r.R; height=$r.B; dpi=[BenchWindow]::GetDpiForWindow($hwnd); display=$screen.DeviceName;
     refreshHz=[BenchGraphics]::Refresh($screen.DeviceName); gpuEngines=$gpuEngines; gpuCounterError=$gpuError;
     displayBounds=@($screen.Bounds.X,$screen.Bounds.Y,$screen.Bounds.Width,$screen.Bounds.Height);
     machineCpuPercent=$load; hostCpuCores= $(if ($elapsed -gt 0) {($hostCpu-$prevHost)/$elapsed} else {0}); buildRunning=$build } | ConvertTo-Json -Depth 5 -Compress
  $prevCpu=$cpu; $prevHost=$hostCpu; $prevAt=$now
  Start-Sleep -Milliseconds 250
}
foreach ($counter in $gpuCounters) { $counter.Dispose() }
