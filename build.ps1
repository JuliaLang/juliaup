&"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\MSBuild\Current\Bin\MSBuild.exe" /property:Configuration=Release /property:Platform=x86 WinJulia.sln
&"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\MSBuild\Current\Bin\MSBuild.exe" /property:Configuration=Release /property:Platform=x64 WinJulia.sln

C:\Users\david\AppData\Local\Programs\Julia-1.6.1\bin\julia.exe .\Juliaup\build.jl
# C:\Users\david\AppData\Local\Programs\Julia-1.6.1-x86\bin\julia.exe .\build.jl

mkdir -Force .\output
mkdir -Force .\build\msix
Remove-Item .\build\msix\*

$versions = Get-Content versions.json | ConvertFrom-Json
[version]$version = $versions.JuliaAppPackage.Version

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayout.xml /op ..\build\msix /pv $version /bv $version
pop-location

Move-Item .\build\msix\*.appxbundle .\output -Force
