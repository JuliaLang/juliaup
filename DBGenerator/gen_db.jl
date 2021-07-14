using JSON, Query, Pkg

Pkg.activate(@__DIR__)
Pkg.instantiate()

function main()
    db_x64 = Dict{String,Any}("AvailableVersions"=>Dict{String,Any}(), "AvailableChannels"=>Dict{String,Any}())
    db_x86 = Dict{String,Any}("AvailableVersions"=>Dict{String,Any}(), "AvailableChannels"=>Dict{String,Any}())

    old_db = JSON.parsefile(joinpath(@__DIR__, "..", "versions.json"), use_mmap=false)

    versions = map(old_db["OptionalJuliaPackages"]) do i
        v = VersionNumber(i["JuliaVersion"])
        return (version=v, major="$(v.major)", minor="$(v.major).$(v.minor)")
    end

    minor_channels = versions |> @groupby(_.minor) |> @map({channel=key(_), version=first(_ |> @orderby_descending(i->i.version)).version})
    major_channels = versions |> @groupby(_.major) |> @map({channel=key(_), version=first(_ |> @orderby_descending(i->i.version)).version})
   
    for v in versions
        db_x64["AvailableVersions"]["$(v.version)+0~x64"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x64/$(v.minor)/julia-$(v.version)-win64.tar.gz")
        db_x64["AvailableVersions"]["$(v.version)+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/$(v.minor)/julia-$(v.version)-win32.tar.gz")
        db_x86["AvailableVersions"]["$(v.version)+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/$(v.minor)/julia-$(v.version)-win32.tar.gz")

        db_x64["AvailableChannels"]["$(v.version)~x64"] = Dict("Version" => "$(v.version)+0~x64")
        db_x64["AvailableChannels"]["$(v.version)~x86"] = Dict("Version" => "$(v.version)+0~x86")
        db_x86["AvailableChannels"]["$(v.version)~x86"] = Dict("Version" => "$(v.version)+0~x86")
        db_x64["AvailableChannels"]["$(v.version)"] = Dict("Version" => "$(v.version)+0~x64")
        db_x86["AvailableChannels"]["$(v.version)"] = Dict("Version" => "$(v.version)+0~x86")
    end

    db_x64["AvailableVersions"]["1.7.0-beta2+0~x64"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x64/1.7/julia-1.7.0-beta2-win64.tar.gz")
    db_x64["AvailableVersions"]["1.7.0-beta2+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/1.7/julia-1.7.0-beta2-win32.tar.gz")
    db_x86["AvailableVersions"]["1.7.0-beta2+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/1.7/julia-1.7.0-beta2-win32.tar.gz")

    db_x64["AvailableVersions"]["1.7.0-beta3+0~x64"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x64/1.7/julia-1.7.0-beta3-win64.tar.gz")
    db_x64["AvailableVersions"]["1.7.0-beta3+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/1.7/julia-1.7.0-beta3-win32.tar.gz")
    db_x86["AvailableVersions"]["1.7.0-beta3+0~x86"] = Dict("Url"=>"https://julialang-s3.julialang.org/bin/winnt/x86/1.7/julia-1.7.0-beta3-win32.tar.gz")

    for c in minor_channels
        db_x64["AvailableChannels"]["$(c.channel)~x64"] = Dict("Version" => "$(c.version)+0~x64")
        db_x64["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)+0~x86")
        db_x86["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)+0~x86")
        db_x64["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)+0~x64")
        db_x86["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)+0~x86")
    end

    for c in major_channels
        db_x64["AvailableChannels"]["$(c.channel)~x64"] = Dict("Version" => "$(c.version)+0~x64")
        db_x64["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)+0~x86")
        db_x86["AvailableChannels"]["$(c.channel)~x86"] = Dict("Version" => "$(c.version)+0~x86")
        db_x64["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)+0~x64")
        db_x86["AvailableChannels"]["$(c.channel)"] = Dict("Version" => "$(c.version)+0~x86")
    end

    db_x64["AvailableChannels"]["release~x64"] = Dict("Version" => "1.6.1+0~x64")
    db_x64["AvailableChannels"]["release~x86"] = Dict("Version" => "1.6.1+0~x86")
    db_x86["AvailableChannels"]["release~x86"] = Dict("Version" => "1.6.1+0~x86")
    db_x64["AvailableChannels"]["release"] = Dict("Version" => "1.6.1+0~x64")
    db_x86["AvailableChannels"]["release"] = Dict("Version" => "1.6.1+0~x86")

    db_x64["AvailableChannels"]["lts~x64"] = Dict("Version" => "1.0.5+0~x64")
    db_x64["AvailableChannels"]["lts~x86"] = Dict("Version" => "1.0.5+0~x86")
    db_x86["AvailableChannels"]["lts~x86"] = Dict("Version" => "1.0.5+0~x86")
    db_x64["AvailableChannels"]["lts"] = Dict("Version" => "1.0.5+0~x64")
    db_x86["AvailableChannels"]["lts"] = Dict("Version" => "1.0.5+0~x86")

    db_x64["AvailableChannels"]["beta~x64"] = Dict("Version" => "1.7.0-beta3+0~x64")
    db_x64["AvailableChannels"]["beta~x86"] = Dict("Version" => "1.7.0-beta3+0~x86")
    db_x86["AvailableChannels"]["beta~x86"] = Dict("Version" => "1.7.0-beta3+0~x86")
    db_x64["AvailableChannels"]["beta"] = Dict("Version" => "1.7.0-beta3+0~x64")
    db_x86["AvailableChannels"]["beta"] = Dict("Version" => "1.7.0-beta3+0~x86")

    db_x64["AvailableChannels"]["rc~x64"] = Dict("Version" => "1.6.1+0~x64")
    db_x64["AvailableChannels"]["rc~x86"] = Dict("Version" => "1.6.1+0~x86")
    db_x86["AvailableChannels"]["rc~x86"] = Dict("Version" => "1.6.1+0~x86")
    db_x64["AvailableChannels"]["rc"] = Dict("Version" => "1.6.1+0~x64")
    db_x86["AvailableChannels"]["rc"] = Dict("Version" => "1.6.1+0~x86")

    mkpath(joinpath(@__DIR__, "..", "build", "versiondb"))

    open(joinpath(@__DIR__, "..", "build", "versiondb", "juliaup-versionsdb-winnt-x64.json"), "w") do f
        JSON.print(f, db_x64, 4)
    end

    open(joinpath(@__DIR__, "..", "build", "versiondb", "juliaup-versionsdb-winnt-arm64.json"), "w") do f
        JSON.print(f, db_x64, 4)
    end

    open(joinpath(@__DIR__, "..", "build", "versiondb", "juliaup-versionsdb-winnt-x86.json"), "w") do f
        JSON.print(f, db_x86, 4)
    end

    open(joinpath(@__DIR__, "..", "build", "versiondb", "juliaup-versionsdb-winnt-arm.json"), "w") do f
        JSON.print(f, db_x86, 4)
    end
end

main()
