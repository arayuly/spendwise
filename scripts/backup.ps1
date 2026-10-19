param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\backups'),
    [switch]$Production
)

$ErrorActionPreference = 'Stop'
$composeArguments = @('compose', '-f', (Join-Path $PSScriptRoot '..\docker-compose.yml'))
if ($Production) {
    $composeArguments += @('-f', (Join-Path $PSScriptRoot '..\docker-compose.production.yml'))
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$outputPath = Join-Path (Resolve-Path -LiteralPath $OutputDirectory).Path ("spendwise-{0}.dump" -f (Get-Date -Format 'yyyyMMdd-HHmmss'))
$containerFile = '/tmp/spendwise-backup-' + [Guid]::NewGuid().ToString('N') + '.dump'
try {
    & docker @composeArguments exec -T db pg_dump -U spendwise -d spendwise --format=custom "--file=$containerFile"
    if ($LASTEXITCODE -ne 0) { throw 'pg_dump failed; backup was not created.' }
    & docker @composeArguments cp "db:$containerFile" $outputPath
    if ($LASTEXITCODE -ne 0) { throw 'Failed to copy the database backup from the container.' }
    Write-Output "Backup created: $outputPath"
} finally {
    & docker @composeArguments exec -T db rm -f -- $containerFile | Out-Null
}
