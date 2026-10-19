param(
    [ValidateSet('run','build','test','check','fmt','clippy')]
    [string]$Task = 'run',
    [switch]$Release,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$ExtraArgs
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    if (Test-Path -LiteralPath "$taskRoot\.tools\cargo\bin\cargo.exe") {
        $env:CARGO_HOME = "$taskRoot\.tools\cargo"
        $env:RUSTUP_HOME = "$taskRoot\.tools\rustup"
        $taskToolchain = "$env:RUSTUP_HOME\toolchains\1.90.0-x86_64-pc-windows-gnu"
        $taskLinker = "$taskToolchain\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained\x86_64-w64-mingw32-gcc.exe"
        $env:Path = "$env:CARGO_HOME\bin;$taskToolchain\bin;$taskToolchain\lib\rustlib\x86_64-pc-windows-gnu\bin;$taskToolchain\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;$env:Path"
        $taskAdapter = "$taskRoot\.tools\linker.exe"
        $taskAdapterSource = "$PSScriptRoot\windows-linker.rs"
        if (!(Test-Path -LiteralPath $taskAdapter) -or (Get-Item -LiteralPath $taskAdapterSource).LastWriteTime -gt (Get-Item -LiteralPath $taskAdapter).LastWriteTime) {
            & "$taskToolchain\bin\rustc.exe" $taskAdapterSource -o $taskAdapter -C link-self-contained=yes -C "linker=$taskLinker"
            if ($LASTEXITCODE -ne 0) { throw 'Could not build local linker adapter' }
        }
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $taskAdapter
        $env:CARGO_ENCODED_RUSTFLAGS = @('-C','linker-flavor=ld.lld','-C','link-self-contained=yes','-C',"dlltool=$taskRoot\.tools\dlltool.exe") -join [char]31
        $taskImportLib = "$taskToolchain\lib\rustlib\x86_64-pc-windows-gnu\lib\self-contained\libshlwapi.a"
        if (!(Test-Path -LiteralPath $taskImportLib)) {
            "LIBRARY shlwapi.dll`nEXPORTS`nAssocQueryStringW" | Set-Content -LiteralPath "$taskRoot\.tools\shlwapi.def" -Encoding ascii
            & "$taskRoot\.tools\dlltool.exe" -d "$taskRoot\.tools\shlwapi.def" -D shlwapi.dll -l $taskImportLib -m i386:x86-64
            if ($LASTEXITCODE -ne 0) { throw 'Could not prepare the local shlwapi import library' }
        }
    }
    $taskArguments = @($Task)
    if ($Task -in @('run','build')) { $taskArguments += @('-p','eda-app') }
    if ($Task -in @('test','check','clippy')) { $taskArguments += '--workspace' }
    if ($Release -and $Task -in @('run','build','test')) { $taskArguments += '--release' }
    if ($ExtraArgs) {
        foreach ($taskArgument in $ExtraArgs) {
            if ($taskArgument -in @('-D','-W','-A') -and '--' -notin $taskArguments) { $taskArguments += '--' }
            $taskArguments += $taskArgument
        }
    }
    & cargo @taskArguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed with exit code $LASTEXITCODE" }
} finally { Pop-Location }
