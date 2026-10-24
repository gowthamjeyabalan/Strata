# Makes the self-signed certificate that signs Strata's MSIX package (once per signing PC), and
# writes its public half to installer/Strata.cer for the release. The private key stays in your
# certificate store (Current User > Personal); it is never written to the repository.
$ErrorActionPreference = 'Stop'
$subject = 'CN=Gowtham Jeyabalan'
$cert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object Subject -eq $subject | Sort-Object NotAfter | Select-Object -Last 1
if (-not $cert) {
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $subject -FriendlyName 'Strata package signing' `
        -CertStoreLocation Cert:\CurrentUser\My -KeyUsage DigitalSignature -KeyAlgorithm RSA -KeyLength 3072 `
        -NotAfter (Get-Date).AddYears(5) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
    Write-Host "Made $subject, valid until $($cert.NotAfter.ToShortDateString())"
}
Export-Certificate -Cert $cert -FilePath (Join-Path $PSScriptRoot 'Strata.cer') | Out-Null
Write-Host "Public certificate written to installer\Strata.cer (thumbprint $($cert.Thumbprint))"
