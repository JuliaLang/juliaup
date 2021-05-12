mkdir -Force output\optional

Remove-Item output\optional\*

Remove-Item output\optional\Microsoft*.appx

$versions = Get-Content versions.json | ConvertFrom-Json

$versions.OptionalJuliaPackages | ForEach-Object -Parallel {
    $juliaVersion = $_.JuliaVersion
    $version = $_.Version
    
    push-location msix
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /o /id "Julia-$juliaVersion-$version" /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv $version /bv $version /bc
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /o /id "Julia-x86-$juliaVersion-$version" /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv $version /bv $version /bc
    pop-location
}

copy-item optionalpackages\Microsoft.VCLibs.x64.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.x86.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.arm.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.arm64.14.00.Desktop.appx output\optional

copy-item optionalpackages\Microsoft.VCLibs.x64.14.00.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.x86.14.00.appx output\optional
