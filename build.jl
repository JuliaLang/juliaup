cd(@__DIR__)

using Pkg

pkg"activate --temp"

Pkg.add("PackageCompiler")

using PackageCompiler

platform = Int==Int64 ? "x64" : "x86"

output_path = joinpath(@__DIR__, "build", "juliaup", platform)

mkpath(output_path)

create_app("Juliaup", output_path, filter_stdlibs=true, force=true)
