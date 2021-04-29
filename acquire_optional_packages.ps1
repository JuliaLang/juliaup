mkdir -Force optionalpackages\win32
mkdir -Force optionalpackages\win64

$versions = Get-Content versions.json | ConvertFrom-Json

$versions.OptionalJuliaPackages | % {[version]$_.JuliaVersion} | ForEach-Object -Parallel {
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x64/$($_.Major).$($_.Minor)/julia-$($_)-win64.tar.gz" -OutFile "optionalpackages/julia-$($_)-win64.tar.gz"
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x86/$($_.Major).$($_.Minor)/julia-$($_)-win32.tar.gz" -OutFile "optionalpackages/julia-$($_)-win32.tar.gz"
    tar -xvzf "optionalpackages/julia-$($_)-win64.tar.gz" -C optionalpackages\win64
    tar -xvzf "optionalpackages/julia-$($_)-win32.tar.gz" -C optionalpackages\win32
}
