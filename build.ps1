&"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\MSBuild\Current\Bin\MSBuild.exe" /property:Configuration=Release /property:Platform=x86 WinJulia.sln
&"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\MSBuild\Current\Bin\MSBuild.exe" /property:Configuration=Release /property:Platform=x64 WinJulia.sln

C:\Users\david\AppData\Local\Programs\Julia-1.6.1-x64\bin\julia.exe .\Juliaup\build.jl
# C:\Users\david\AppData\Local\Programs\Julia-1.6.1-x86\bin\julia.exe .\build.jl

mkdir -Force .\output
mkdir -Force .\build\msix
mkdir -Force build\downloads
mkdir -Force build\juliaversions\x64
mkdir -Force build\juliaversions\x86
Remove-Item .\build\msix\*

$versions = Get-Content versions.json | ConvertFrom-Json
[version]$version = $versions.JuliaAppPackage.Version
[version]$bundledVersion = $versions.JuliaAppPackage.BundledJuliaVersion
$bundledVersionAsString = $versions.JuliaAppPackage.BundledJuliaVersion

if (-Not (Test-Path "build\downloads\julia-$($bundledVersionAsString)-win64.tar.gz"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x64/$($bundledVersion.Major).$($bundledVersion.Minor)/julia-$($bundledVersionAsString)-win64.tar.gz" -OutFile "build\downloads\julia-$($bundledVersionAsString)-win64.tar.gz"
}

if (-Not (Test-Path "build\downloads\julia-$($bundledVersionAsString)-win32.tar.gz"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x86/$($bundledVersion.Major).$($bundledVersion.Minor)/julia-$($bundledVersionAsString)-win32.tar.gz" -OutFile "build\downloads\julia-$($bundledVersionAsString)-win32.tar.gz"
}

if (-Not (Test-Path "build\juliaversions\x64\julia-$($bundledVersionAsString)"))
{
    tar -xvzf "build\downloads\julia-$($bundledVersion)-win64.tar.gz" -C build\juliaversions\x64
}

if (-Not (Test-Path "build\juliaversions\x86\julia-$($bundledVersionAsString)"))
{
    tar -xvzf "build\downloads\julia-$($bundledVersion)-win32.tar.gz" -C build\juliaversions\x86
}

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\build\msix /pv $version /bv $version
pop-location

Move-Item .\build\msix\*.appxbundle .\output -Force
