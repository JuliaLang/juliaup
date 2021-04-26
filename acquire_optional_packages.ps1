mkdir -Force optionalpackages

@('1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0', '1.6.1') | ForEach-Object -Parallel {
    [version]$VERSION = $_
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x64/$($VERSION.Major).$($VERSION.Minor)/julia-$($_)-win64.tar.gz" -OutFile "optionalpackages/julia-$($_)-win64.tar.gz"
    tar -xvzf "optionalpackages/julia-$($_)-win64.tar.gz" -C optionalpackages
}
