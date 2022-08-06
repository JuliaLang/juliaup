
$versions = Get-Content versions.json | ConvertFrom-Json
[version]$bundledVersion = $versions.JuliaAppPackage.BundledJuliaVersion
$bundledVersionAsString = $versions.JuliaAppPackage.BundledJuliaVersion

mkdir -Force target\bundledjulia\downloads
mkdir -Force target\bundledjulia\extracted

if (-Not (Test-Path "target\bundledjulia\downloads\julia-$($bundledVersionAsString)-win64.tar.gz"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x64/$($bundledVersion.Major).$($bundledVersion.Minor)/julia-$($bundledVersionAsString)-win64.tar.gz" -OutFile "target\bundledjulia\downloads\julia-$($bundledVersionAsString)-win64.tar.gz"
    mkdir -Force target\bundledjulia\extracted\x64
    Remove-Item target\bundledjulia\extracted\x64\* -Force -Recurse    
    tar -xvzf "target\bundledjulia\downloads\julia-$($bundledVersion)-win64.tar.gz" -C target\bundledjulia\extracted\x64 --strip-components=1
}

if (-Not (Test-Path "target\bundledjulia\downloads\julia-$($bundledVersionAsString)-win32.tar.gz"))
{
    Invoke-WebRequest "https://julialang-s3.julialang.org/bin/winnt/x86/$($bundledVersion.Major).$($bundledVersion.Minor)/julia-$($bundledVersionAsString)-win32.tar.gz" -OutFile "target\bundledjulia\downloads\julia-$($bundledVersionAsString)-win32.tar.gz"
    mkdir -Force target\bundledjulia\extracted\x86
    Remove-Item target\bundledjulia\extracted\x86\* -Force -Recurse
    tar -xvzf "target\bundledjulia\downloads\julia-$($bundledVersion)-win32.tar.gz" -C target\bundledjulia\extracted\x86 --strip-components=1
}

push-location deploy\msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\..\target\msix
#  /pv $version /bv $version
pop-location

# Move-Item .\build\msix\*.appxbundle .\output -Force
