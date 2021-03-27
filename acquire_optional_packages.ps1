Invoke-WebRequest https://julialang-s3.julialang.org/bin/winnt/x64/1.5/julia-1.5.0-rc1-win64.tar.gz -OutFile optionalpackages/julia-1.5.0-rc1-win64.tar.gz

tar -xvzf optionalpackages/julia-1.5.0-rc1-win64.tar.gz -C optionalpackages
