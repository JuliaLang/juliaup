mkdir -Force output\optional

Remove-Item output\optional\*

Remove-Item output\optional\Microsoft*.appx

$versions = Get-Content versions.json | ConvertFrom-Json

$versions.OptionalJuliaPackages | ForEach-Object -Parallel {
    $juliaVersion = $_.JuliaVersion
    $version = $_.Version

    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win64\julia-$juliaVersion\bin\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win64\julia-$juliaVersion\bin\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win64\julia-$juliaVersion\libexec\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win64\julia-$juliaVersion\libexec\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win64\julia-$juliaVersion\lib\julia\*.dll"

    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win32\julia-$juliaVersion\bin\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win32\julia-$juliaVersion\bin\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win32\julia-$juliaVersion\libexec\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win32\julia-$juliaVersion\libexec\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 "optionalpackages\win32\julia-$juliaVersion\lib\julia\*.dll"

    push-location msix
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /o /id "Julia-$juliaVersion-$version" /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv $version /bv $version /bc
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /o /id "Julia-x86-$juliaVersion-$version" /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv $version /bv $version /bc
    pop-location
}

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 467A053795F772FC96BC766AD85D2C039E4DF9B3 output\optional\*

copy-item optionalpackages\Microsoft.VCLibs.x64.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.x86.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.arm.14.00.Desktop.appx output\optional
copy-item optionalpackages\Microsoft.VCLibs.arm64.14.00.Desktop.appx output\optional

copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs\14.0\Appx\Retail\x64\Microsoft.VCLibs.x64.14.00.appx" output\optional
copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs\14.0\Appx\Retail\x86\Microsoft.VCLibs.x86.14.00.appx" output\optional
