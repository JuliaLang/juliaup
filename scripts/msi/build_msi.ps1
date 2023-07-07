if (Test-Path -Path $PSScriptRoot\..\..\target\x86_64-pc-windows-gnu\release) {
    Write-Output "Creating x64 installer..."
    md $PSScriptRoot\..\..\target\msi\x86_64-pc-windows-gnu\release -force | Out-Null
    Copy-Item $PSScriptRoot\..\..\target\x86_64-pc-windows-gnu\release\*.exe $PSScriptRoot\..\..\target\msi\x86_64-pc-windows-gnu\release
    Copy-Item $PSScriptRoot\..\..\deploy\msi\License.rtf $PSScriptRoot\..\..\target\msi\x86_64-pc-windows-gnu\release
    Copy-Item $PSScriptRoot\..\..\deploy\msi\Bitmaps $PSScriptRoot\..\..\target\msi\x86_64-pc-windows-gnu\release -Recurse -Force
    wix build -ext WixToolset.UI.wixext $PSScriptRoot\..\..\deploy\msi\Julia.wxs -b $PSScriptRoot\..\..\target\msi\x86_64-pc-windows-gnu\release -arch x64 -o $PSScriptRoot\..\..\target\msi\Julia-x64.msi
}
else {
    Write-Output "Skipping x64 installer."
}

if (Test-Path -Path $PSScriptRoot\..\..\target\i686-pc-windows-gnu\release) {
    Write-Output "Creating x86 installer..."
    md $PSScriptRoot\..\..\target\msi\i686-pc-windows-gnu\release -force | Out-Null
    Copy-Item $PSScriptRoot\..\..\target\i686-pc-windows-gnu\release\*.exe $PSScriptRoot\..\..\target\msi\i686-pc-windows-gnu\release
    Copy-Item $PSScriptRoot\..\..\deploy\msi\License.rtf $PSScriptRoot\..\..\target\msi\i686-pc-windows-gnu\release
    Copy-Item $PSScriptRoot\..\..\deploy\msi\Bitmaps $PSScriptRoot\..\..\target\msi\i686-pc-windows-gnu\release -Recurse -Force
    wix build -ext WixToolset.UI.wixext $PSScriptRoot\..\..\deploy\msi\Julia.wxs -b $PSScriptRoot\..\..\target\msi\i686-pc-windows-gnu\release -arch x86 -o $PSScriptRoot\..\..\target\msi\Julia-x86.msi
}
else {
    Write-Output "Skipping x86 installer."
}
