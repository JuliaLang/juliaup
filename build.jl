cd(@__DIR__)

using Pkg

pkg"activate --temp"

Pkg.add("PackageCompiler")

using PackageCompiler

create_app("Juliaup", "MyAppCompiled", filter_stdlibs=true, force=true)
