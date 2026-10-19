# Prints random, URL-safe values for .env. Never redirect the output into Git.
$ErrorActionPreference = 'Stop'
function New-RandomHex([int]$ByteCount) {
    $bytes = New-Object byte[] $ByteCount
    $generator = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try { $generator.GetBytes($bytes) } finally { $generator.Dispose() }
    return [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant()
}
Write-Output ('JWT_SECRET=' + (New-RandomHex 48))
Write-Output ('POSTGRES_PASSWORD=' + (New-RandomHex 32))
Write-Output ('POSTGRES_ADMIN_PASSWORD=' + (New-RandomHex 32))
