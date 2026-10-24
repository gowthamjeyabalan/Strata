# Builds Strata's MSIX installer from target\release\strata.exe and signs it.
#   cargo build --release
#   powershell -ExecutionPolicy Bypass -File installer\build-msix.ps1
# Writes target\Strata-<version>.msix. The version comes from Cargo.toml unless -Version is given.
# Signing needs the certificate made by installer\make-cert.ps1.
param([string]$Version)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$sdk = (Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\makeappx.exe" | Sort-Object FullName | Select-Object -Last 1).Directory.FullName
if (-not $sdk) { throw 'The Windows SDK (makeappx.exe) was not found' }
if (-not $Version) {
    $Version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
}
$exe = "$root\target\release\strata.exe"
if (-not (Test-Path $exe)) { throw 'Build it first: cargo build --release' }

# The package's contents: the app, its tiles and icons, and the manifest with the version.
$stage = "$root\target\msix"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item $stage -ItemType Directory | Out-Null
Copy-Item $exe $stage
Copy-Item "$PSScriptRoot\msix\Assets" "$stage\Assets" -Recurse
(Get-Content "$PSScriptRoot\msix\AppxManifest.xml" -Raw).Replace('$VERSION$', "$Version.0") | Set-Content "$stage\AppxManifest.xml" -Encoding utf8

# A resource index, so Windows picks the right icon for each size and theme.
& "$sdk\makepri.exe" createconfig /cf "$stage\priconfig.xml" /dq en-US /o | Out-Null
& "$sdk\makepri.exe" new /pr $stage /cf "$stage\priconfig.xml" /of "$stage\resources.pri" /o | Out-Null
if ($LASTEXITCODE) { throw 'makepri failed' }
Remove-Item "$stage\priconfig.xml"

$out = "$root\target\Strata-$Version.msix"
& "$sdk\makeappx.exe" pack /d $stage /p $out /o | Out-Null
if ($LASTEXITCODE) { throw 'makeappx failed' }

$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object Subject -eq 'CN=Gowtham Jeyabalan' | Sort-Object NotAfter | Select-Object -Last 1
if (-not $cert) { throw 'No signing certificate: run installer\make-cert.ps1 first' }
& "$sdk\signtool.exe" sign /fd SHA256 /sha1 $cert.Thumbprint $out | Out-Null
if ($LASTEXITCODE) { throw 'signtool failed' }
Write-Host "Built and signed $out"
