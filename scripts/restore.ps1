param(
    [Parameter(Mandatory = $true)][string]$BackupFile,
    [switch]$Production
)

$ErrorActionPreference = 'Stop'
$backupPath = (Resolve-Path -LiteralPath $BackupFile).Path
if (-not (Test-Path -LiteralPath $backupPath -PathType Leaf)) { throw 'BackupFile must be an existing file.' }
$composeArguments = @('compose', '-f', (Join-Path $PSScriptRoot '..\docker-compose.yml'))
if ($Production) {
    $composeArguments += @('-f', (Join-Path $PSScriptRoot '..\docker-compose.production.yml'))
}
$containerFile = '/tmp/spendwise-restore-' + [Guid]::NewGuid().ToString('N') + '.dump'
try {
    # The operator must stop app first. PostgreSQL stays running for restoration.
    & docker @composeArguments cp $backupPath "db:$containerFile"
    if ($LASTEXITCODE -ne 0) { throw 'Failed to copy backup into PostgreSQL container.' }
    & docker @composeArguments exec -T db pg_restore -U spendwise -d spendwise --clean --if-exists --no-owner --exit-on-error --single-transaction $containerFile
    if ($LASTEXITCODE -ne 0) { throw 'Restore failed and its transaction was rolled back.' }
    Write-Output 'Database restored. Start app and verify /health/ready.'
} finally {
    & docker @composeArguments exec -T db rm -f -- $containerFile | Out-Null
}
