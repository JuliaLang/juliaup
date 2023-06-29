# md .\target\msi\i686-pc-windows-gnu\release -force
md .\target\msi\x86_64-pc-windows-gnu\release -force
Copy-Item .\target\x86_64-pc-windows-gnu\release\*.exe .\target\msi\x86_64-pc-windows-gnu\release
# Copy-Item .\target\i686-pc-windows-gnu\release\*.exe .\target\msi\i686-pc-windows-gnu\release
# Copy-Item .\deploy\msi\License.rtf .\target\msi\i686-pc-windows-gnu\release
Copy-Item .\deploy\msi\License.rtf .\target\msi\x86_64-pc-windows-gnu\release
# wix build -ext WixToolset.UI.wixext .\deploy\msi\Julia.wxs -b .\target\msi\i686-pc-windows-gnu\release -arch x86 -o target\msi\Julia-x86.msi
wix build -ext WixToolset.UI.wixext .\deploy\msi\Julia.wxs -b .\target\msi\x86_64-pc-windows-gnu\release -arch x64 -o target\msi\Julia-x64.msi
