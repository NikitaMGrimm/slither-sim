param([Parameter(Mandatory=$true)][string]$Archive,[Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$root = [IO.Path]::GetFullPath($Destination) + [IO.Path]::DirectorySeparatorChar
if (Test-Path -LiteralPath $Destination) { throw 'Extraction destination must be new' }
$zip = [IO.Compression.ZipFile]::OpenRead($Archive)
try {
  $total = 0L
  foreach ($entry in $zip.Entries) {
    $target = [IO.Path]::GetFullPath([IO.Path]::Combine($root,$entry.FullName))
    if (!$target.StartsWith($root,[StringComparison]::OrdinalIgnoreCase) -or $entry.FullName.Contains(':')) { throw 'Unsafe archive path' }
    if ((($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000) { throw 'Archive symlink rejected' }
    $total += $entry.Length
    if ($total -gt 512MB) { throw 'Mod package exceeds 512 MiB uncompressed bound' }
  }
  [IO.Compression.ZipFile]::ExtractToDirectory($Archive,$Destination)
} finally { $zip.Dispose() }
