param([string]$Destination = (Join-Path $PSScriptRoot '..\.env'))

$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $Destination) { throw 'Destination already exists. Keep existing secrets and edit the file explicitly.' }
$template = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\.env.example') -Raw
$values = @{}
foreach ($entry in @(@('JWT_SECRET', 48), @('POSTGRES_PASSWORD', 32), @('POSTGRES_ADMIN_PASSWORD', 32))) {
    $bytes = New-Object byte[] $entry[1]
    $generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try { $generator.GetBytes($bytes) } finally { $generator.Dispose() }
    $values[$entry[0]] = [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant()
    $template = $template.Replace(($entry[0] + '=' + "`n"), ($entry[0] + '=' + $values[$entry[0]] + "`n"))
    $template = $template.Replace(($entry[0] + '=' + "`r`n"), ($entry[0] + '=' + $values[$entry[0]] + "`r`n"))
}
$template = $template.Replace('SET_GENERATED_POSTGRES_PASSWORD', $values['POSTGRES_PASSWORD'])
$encoding = New-Object System.Text.UTF8Encoding $false
$fileStream = [System.IO.File]::Open([System.IO.Path]::GetFullPath($Destination), [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
try {
    $contentBytes = $encoding.GetBytes($template)
    $fileStream.Write($contentBytes, 0, $contentBytes.Length)
    $fileStream.Flush($true)
} finally {
    $fileStream.Dispose()
}
Write-Output "Created $Destination with independent random secrets. The file is ignored by Git."
