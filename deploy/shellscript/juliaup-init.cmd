<# :
:: Powershell trampoline
@echo off
setlocal 
set "_arg1=%~1" &set "_arg2=%~2" &set "_arg3=%~3" &set "_arg4=%~4" &set "_arg5=%~5" &set "_arg6=%~6" &set "_arg7=%~7" &set "_arg8=%~8" &set "_arg9=%~9"
powershell -NoLogo -NoP -Exec ByPass "iex (Get-Content '%~f0' -Raw)"

:: Pause if this is a cmd pop-up from Explorer
if /i "%comspec% /c __%~0_ _" equ "%cmdcmdline:"=_%" pause
goto :EOF
#>


# Choose between Powershell or Batch arguments
if(-not $args){
    $cmdargs = @($env:_arg1, $env:_arg2, $env:_arg3, $env:_arg4, $env:_arg5, $env:_arg6, $env:_arg7, $env:_arg8, $env:_arg9).Where({ $_ -ne $null})
} else {
    $cmdargs = $args
}

function Remove-FromUserPath {
    param (
        [Parameter(Mandatory=$true)]
        [ValidateNotNullOrEmpty()]
        [string] 
        $dir
    )

    $dir = [io.path]::GetFullPath($dir)
    $path = [Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
    if (";$path;".Contains(";$dir;")) {
        $path=((";$path;").replace(";$dir;", ";")).Trim(';')
        [Environment]::SetEnvironmentVariable("PATH", $path, [EnvironmentVariableTarget]::User)
        return
    }
}


function Add-ToUserPath {
    param (
        [Parameter(Mandatory=$true)]
        [ValidateNotNullOrEmpty()]
        [string] 
        $dir
    )

    $dir = [io.path]::GetFullPath($dir)
    $path = [Environment]::GetEnvironmentVariable("PATH", [System.EnvironmentVariableTarget]::User)
    if (!(";$path;".Contains(";$dir;"))) {
        [Environment]::SetEnvironmentVariable("PATH", "$dir;$path", [EnvironmentVariableTarget]::User)
        return
    }
}

function Set-HighestEncryption {
    # Implementation adapted from the Chocolatey installer script,
    # see: https://chocolatey.org/install.ps1

    # Attempt to set highest encryption available for SecurityProtocol.
    # PowerShell will not set this by default (until maybe .NET 4.6.x). This
    # will typically produce a message for PowerShell v2 (just an info message
    # though)
    $OldSPM = [System.Net.ServicePointManager]::SecurityProtocol
    try {
        # Set TLS 1.2 (3072) which is currently the highest protocol enabled on
        # static.rust-lang.org. Favor TLS 1.3 (12288) with a TLS 1.2 fallback
        # when 1.3 is supported. Use integers because the enumeration values
        # for TLS 1.2 won't exist in .NET 4.0, even though they are addressable
        # if .NET 4.5+ is installed (.NET 4.5 is an in-place upgrade).
        [System.Net.ServicePointManager]::SecurityProtocol = 3072
    } catch {
        Write-Failure @'
rustup: Unable to set PowerShell to use TLS 1.2 due to old .NET Framework
installed. If you see underlying connection closed or trust errors, you may
need to do one or more of the following: (1) upgrade to .NET Framework 4.5+ and
PowerShell v3+, (2) download an alternative install method from
https://rustup.rs/.
'@
    } finally {
        [System.Net.ServicePointManager]::SecurityProtocol = $OldSPM
    }
}

$JULIAUP_SERVER='https://github.com/JuliaLang/juliaup/releases/latest/download/'
$JULIAUP_INSTALL_DIR=(Resolve-Path '~/.juliaup')

$usage = @'
juliaup-init: the installer for juliaup

USAGE:
    juliaup-init [FLAGS] [OPTIONS]

FLAGS:
    -y, --yes               Answer y to all confirmation prompts.
        --no-add-to-path    Don't add to user PATH environment variable
    -h, --help              Prints help information

OPTIONS:
    -p, --path              Custom install path

'@

# Display help
if(($cmdargs.Contains('-h')) -or ($cmdargs.Contains('--help'))){
    Write-Host $usage
    Exit
}


$dirprompt = @"

Install juliaup to $JULIAUP_INSTALL_DIR ?
    [y] to confirm
    [n] to abort
    ... Or enter a custom location
"@


$pathprompt = @'

Add juliaup.exe and julia.exe to user $PATH variable?
    [y] to confirm
    [n] to deny
'@


# User-defined path
if(($cmdargs.Contains('-p')) -or ($cmdargs.Contains('--path'))){
    $i = [array]::indexof($cmdargs, '-p')
    if($i -lt 0){$i = [array]::indexof($cmdargs, '--path')}
    $JULIAUP_INSTALL_DIR = $cmdargs[$i+1]

# Ask user for path
} elseif((-not $cmdargs.Contains('-y')) -and (-not $cmdargs.Contains('--yes'))) {
    $ans = ""
    Write-Host $dirprompt
    while($ans -eq ""){
        $ans = Read-Host -Prompt ">>> [y/n/...]"
        if ($ans -ne "") {
            if (($ans -eq "Y") -or ($ans -eq "y")){
                #nothing
            } elseif (($ans -eq "N") -or ($ans -eq "n")) {
                Exit
            } else {
                $JULIAUP_INSTALL_DIR = $ans
            }
        }
    }
}

$doAddPath = $true
if($cmdargs.Contains('--no-add-to-path')){
    $doAddPath = $false
} else {
    if((-not $cmdargs.Contains('-y')) -and (-not $cmdargs.Contains('--yes'))) {
        $ans = ""
        Write-Host $pathprompt
        while(($ans -ne "Y") -and ($ans -ne "y") -and ($ans -ne "N") -and ($ans -ne "n")){
            $ans = Read-Host -Prompt ">>> [y/n]"
        
            if (($ans -eq "N") -or ($ans -eq "n")){
                $doAddPath = $false
            }
        }
    }
}


# Create temp directory
$tempFolderPath = Join-Path $Env:Temp $(New-Guid)
New-Item -Type Directory -Force -Path $tempFolderPath | Out-Null


$arch = 'x86_64-pc-windows-msvc'
if ([IntPtr]::size -eq 4){
    $arch = 'i686-pc-windows-msvc'
}

$url = "$JULIAUP_SERVER/${arch}.zip"
$dst = "$tempFolderPath/${arch}.zip"
$dstdir = "$tempFolderPath/${arch}"
Write-Host ""
Write-Host "info: downloading installer"

# Suppress progress bar
$tmppref = $global:ProgressPreference
$global:ProgressPreference = "SilentlyContinue"

Set-HighestEncryption
try {
    Invoke-WebRequest $url -OutFile $dst
}
catch {
    (New-Object System.Net.WebClient).DownloadFile($url, $dst)
}

Expand-Archive -Path $dst -DestinationPath $dstdir

# Restore progress bar
$global:ProgressPreference = $tmppref


# Move extracted to the final location
$binDir = "$JULIAUP_INSTALL_DIR/bin"
Remove-Item -Recurse -Force $binDir -ErrorAction Ignore
New-Item -Type Directory -Force -Path $binDir | Out-Null

Get-ChildItem -Path $dstdir -Recurse -File |
    Move-Item -Destination {
      Join-Path $binDir (Split-Path $_ -leaf)
    }


# Clear path from user PATH; then add it back
Remove-FromUserPath "$binDir"
if ($doAddPath){
    Add-ToUserPath "$binDir"
    Write-Host 'Close and re-open any terminal to reload $PATH variable'
}

Remove-Item -Recurse -Force $tempFolderPath -ErrorAction Ignore

# TODO: how can we reliably symlink on Windows?
Copy-Item -Path "$binDir/julialauncher.exe" -Destination "$binDir/julia.exe" -Force

# Trigger Julia download
& "$binDir/julia.exe" -e nothing

Write-Host 'Run `juliaup --help` for help'