mkdir -Force output\optional

Remove-Item output\optional\*

@('1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0', '1.6.1') | ForEach-Object -Parallel {
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 "optionalpackages\julia-$($_)\bin\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 "optionalpackages\julia-$($_)\bin\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 "optionalpackages\julia-$($_)\libexec\*.exe"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 "optionalpackages\julia-$($_)\libexec\*.dll"
    &"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 "optionalpackages\julia-$($_)\lib\julia\*.dll"
}

push-location msix
&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\MakeAppx.exe" build /f PackagingLayoutOptionalPackages.xml /op ..\output\optional /pv 1.0.0.0 /bv 1.0.0.0
pop-location

&"C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool" sign /fd SHA256 /sha1 E70A5E7F058A0E4FCAAC9CC604C44EC8588D1C59 output\optional\*

copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs.Desktop\14.0\Appx\Retail\x64\Microsoft.VCLibs.x64.14.00.Desktop.appx" output\optional
copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs.Desktop\14.0\Appx\Retail\x86\Microsoft.VCLibs.x86.14.00.Desktop.appx" output\optional

copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs\14.0\Appx\Retail\x64\Microsoft.VCLibs.x64.14.00.appx" output\optional
copy-item "C:\Program Files (x86)\Microsoft SDKs\Windows Kits\10\ExtensionSDKs\Microsoft.VCLibs\14.0\Appx\Retail\x86\Microsoft.VCLibs.x86.14.00.appx" output\optional
