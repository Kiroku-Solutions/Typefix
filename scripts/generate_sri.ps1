param(
    [string]$Directory = "pkg"
)

if (-not (Test-Path $Directory)) {
    Write-Host "Directory $Directory does not exist."
    exit 1
}

$files = Get-ChildItem -Path $Directory -File

foreach ($file in $files) {
    $bytes = [System.IO.File]::ReadAllBytes($file.FullName)
    $sha384 = [System.Security.Cryptography.SHA384]::Create()
    $hashBytes = $sha384.ComputeHash($bytes)
    $hashBase64 = [Convert]::ToBase64String($hashBytes)
    Write-Host "$($file.Name): sha384-$hashBase64"
}
