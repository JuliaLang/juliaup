using Pkg
Pkg.activate(@__DIR__)
Pkg.instantiate()

using JSON, Query, OrderedCollections, Downloads

function remove_prefix(s, prefix)
    startswith(s, prefix) || error("Invalid URL.")

    return s[length(prefix)+1:end]
end

function triplet2channel(triplet)
    if triplet=="x86_64-w64-mingw32"
        "x64"
    elseif triplet=="i686-w64-mingw32"
        "x86"
    elseif triplet=="x86_64-apple-darwin14"
        "x64"
    elseif triplet=="aarch64-apple-darwin14"
        "aarch64"
    elseif triplet=="x86_64-linux-gnu"
        "x64"
    elseif triplet=="i686-linux-gnu"
        "x86"
    elseif triplet=="aarch64-linux-gnu"
        "aarch64"
    else
        error("Unknown platform.")
    end
end

function get_available_versions(data, platform)
    lts_major = 1
    lts_minor = 6

    # Make sure the vector here is sorted by priority!
    platforms_to_include = if platform=="x86_64-w64-mingw32"
        ["x86_64-w64-mingw32", "i686-w64-mingw32"]
    elseif platform=="i686-w64-mingw32"
        ["i686-w64-mingw32"]
    elseif platform=="x86_64-apple-darwin14"
        ["x86_64-apple-darwin14"]
    elseif platform=="aarch64-apple-darwin14"
        ["aarch64-apple-darwin14", "x86_64-apple-darwin14"]
    elseif platform=="x86_64-linux-gnu"
        ["x86_64-linux-gnu", "i686-linux-gnu"]
    elseif platform=="i686-linux-gnu"
        ["i686-linux-gnu"]
    elseif platform=="aarch64-linux-gnu"
        ["aarch64-linux-gnu"]
    else
        error("Unknown platform.")
    end

    all_versions = data |> pairs |> @map(VersionNumber(_[1])) |> @orderby(_) |> collect
      
    available_versions = data |>
        pairs |>
        @map({version=_[1], stable=_[2]["stable"], files=_[2]["files"]}) |>
        @mapmany(_.files, {_.version, _.stable, extension=__["extension"], triplet=__["triplet"], kind=__["kind"], arch=__["arch"], sha256=__["sha256"], size=__["size"], url=__["url"], os=__["os"], asc=get(__, "asc", "")}) |>
        @filter(_.extension=="tar.gz" && _.kind=="archive" && _.triplet in platforms_to_include) |>
        @mutate(version=VersionNumber(_.version), url_path=remove_prefix(_.url, "https://julialang-s3.julialang.org/")) |>
        @orderby(_.version) |>
        @thenby(_.triplet) |>
        @map("$(_.version)+0.$(_.triplet)" => OrderedDict("UrlPath" => _.url_path)) |>
        OrderedDict

    available_channels = Dict()

    # Add all full versions
    for v in all_versions
        for p in platforms_to_include
            if haskey(available_versions, "$(v)+0.$p")
                available_channels["$v"] = Dict("Version"=>"$(v)+0.$p")
                break
            end
        end

        for p in platforms_to_include
            if haskey(available_versions, "$(v)+0.$p")
                available_channels["$v~$(triplet2channel(p))"] = Dict("Version"=>"$(v)+0.$p")
            end
        end
    end

    # Add all minor and major versions
    minor_channels = all_versions |>
        @map({major=convert(Int, _.major), minor=convert(Int, _.minor), version=_}) |>
        @groupby({_.major, _.minor}) |>
        @map({key(_)..., stable_versions=filter(i->isempty(i.prerelease), _.version), prerelease_versions=filter(i->!isempty(i.prerelease), _.version)}) |>
        @map({_.major, _.minor, version=isempty(_.stable_versions) ? maximum(_.prerelease_versions) : maximum(_.stable_versions)})

    for v in minor_channels
        for p in platforms_to_include
            if haskey(available_versions, "$(v.version)+0.$p")
                available_channels["$(v.major).$(v.minor)"] = Dict("Version"=>"$(v.version)+0.$p")
                break
            end
        end

        for p in platforms_to_include
            if haskey(available_versions, "$(v.version)+0.$p")
                available_channels["$(v.major).$(v.minor)~$(triplet2channel(p))"] = Dict("Version"=>"$(v.version)+0.$p")
            end
        end
    end
    
    major_channels = all_versions |>
        @map({major=convert(Int, _.major), version=_}) |>
        @groupby({_.major}) |>
        @map({key(_)..., stable_versions=filter(i->isempty(i.prerelease), _.version), prerelease_versions=filter(i->!isempty(i.prerelease), _.version)}) |>
        @map({_.major, version=isempty(_.stable_versions) ? maximum(_.prerelease_versions) : maximum(_.stable_versions)})

    for v in major_channels
        for p in platforms_to_include
            if haskey(available_versions, "$(v.version)+0.$p")
                available_channels["$(v.major)"] = Dict("Version"=>"$(v.version)+0.$p")
                break
            end
        end

        for p in platforms_to_include
            if haskey(available_versions, "$(v.version)+0.$p")
                available_channels["$(v.major)~$(triplet2channel(p))"] = Dict("Version"=>"$(v.version)+0.$p")
            end
        end
    end

    release_version = all_versions |>
        @filter(isempty(_.prerelease)) |>
        maximum

    for p in platforms_to_include
        if haskey(available_versions, "$release_version+0.$p")
            available_channels["release"] = Dict("Version"=>"$release_version+0.$p")
            break
        end
    end

    for p in platforms_to_include
        if haskey(available_versions, "$release_version+0.$p")
            available_channels["release~$(triplet2channel(p))"] = Dict("Version"=>"$release_version+0.$p")
        end
    end        

    lts_version = all_versions |>
        @filter(isempty(_.prerelease) && _.major==lts_major && _.minor==lts_minor) |>
        maximum

    for p in platforms_to_include
        if haskey(available_versions, "$lts_version+0.$p")
            available_channels["lts"] = Dict("Version"=>"$lts_version+0.$p")
            break
        end
    end

    for p in platforms_to_include
        if haskey(available_versions, "$lts_version+0.$p")
            available_channels["lts~$(triplet2channel(p))"] = Dict("Version"=>"$lts_version+0.$p")
        end
    end        
    

    rc_version = all_versions |>
        @filter(!isempty(_.prerelease) && startswith(_.prerelease[1], "rc")) |>
        maximum    
    if rc_version < release_version
        rc_version = release_version
    end

    for p in platforms_to_include
        if haskey(available_versions, "$rc_version+0.$p")
            available_channels["rc"] = Dict("Version"=>"$rc_version+0.$p")
            break
        end
    end

    for p in platforms_to_include
        if haskey(available_versions, "$rc_version+0.$p")
            available_channels["rc~$(triplet2channel(p))"] = Dict("Version"=>"$rc_version+0.$p")
        end
    end        

    beta_version = all_versions |>
        @filter(!isempty(_.prerelease) && startswith(_.prerelease[1], "beta")) |>
        maximum
    if beta_version < rc_version
        beta_version = rc_version
    end

    for p in platforms_to_include
        if haskey(available_versions, "$beta_version+0.$p")
            available_channels["beta"] = Dict("Version"=>"$beta_version+0.$p")
            break
        end
    end

    for p in platforms_to_include
        if haskey(available_versions, "$beta_version+0.$p")
            available_channels["beta~$(triplet2channel(p))"] = Dict("Version"=>"$beta_version+0.$p")
        end
    end

    available_channels = available_channels |>
        pairs |>
        @orderby(_[1]) |>
        @map(_[1] => _[2]) |>
        OrderedDict

    return OrderedDict{String,Any}("AvailableVersions" => available_versions, "AvailableChannels" => available_channels)
end

function get_current_versions_json(download_folder)
    mkpath(download_folder)
    Downloads.download("https://julialang-s3.julialang.org/bin/versions.json", joinpath(download_folder, "versions.json"))
end

function get_current_versiondb_version()
    return VersionNumber(chomp(String(take!(Downloads.download("https://julialang-s3.julialang.org/juliaup/DBVERSION",IOBuffer())))))
end

function get_old_versions(db_version, platform, temp_path)
    mkpath(temp_path)

    return nothing
end

function main_impl(temp_path)
    get_current_versions_json(joinpath(temp_path, "officialversionsjson"))

    versions_json_data = JSON.parsefile(joinpath(temp_path, "officialversionsjson", "versions.json"))

    platforms = [
        "x86_64-linux-gnu",
        "i686-linux-gnu",
        "x86_64-apple-darwin14",
        "x86_64-w64-mingw32",
        "i686-w64-mingw32",
        "aarch64-linux-gnu",
        "aarch64-apple-darwin14",
    ]

    new_version_dbs = platforms |>
        @map(_ => get_available_versions(versions_json_data, _)) |>
        OrderedDict

    old_db_version = get_current_versiondb_version()

    old_version_dbs = platforms |>
        @map(_ => get_old_versions(old_db_version, _, joinpath(temp_path, "oldversiondbs"))) |>
        OrderedDict

    update_needed = false

    if new_version_dbs != old_version_dbs
        update_needed = true
    end

    println(stderr, "update_needed = $update_needed")

    # TODO Remove this once things are stable
    update_needed = true

    if update_needed
        new_version = VersionNumber(old_db_version.major, old_db_version.minor, old_db_version.patch + 1)

        path_for_new_versiondbs = joinpath(temp_path, "newversiondbs", "versiondb")
        mkpath(path_for_new_versiondbs)

        # First add the new version
        for p in platforms
            new_version_dbs[p]["Version"] = string(new_version)
        end

        for p in platforms
            open(joinpath(path_for_new_versiondbs, "versiondb-$new_version-$p.json"), "w") do f
                JSON.print(f, new_version_dbs[p], 4)
            end
        end

        path_for_new_version_file = joinpath(temp_path, "versionfile")
        mkpath(path_for_new_version_file)
        open(joinpath(path_for_new_version_file, "DBVERSION"), "w") do f
            println(f, new_version)
        end
    end
    
    return (update_needed=update_needed,)
end

function main(temp_path=nothing)
    ret = nothing
    if isnothing(temp_path)
        mktempdir() do temp_path
            ret = main_impl(temp_path)
        end
    else
        rm(temp_path, force=true, recursive=true)
        ret = main_impl(temp_path)
    end

    return ret
end

ret = main(length(ARGS)>0 ? ARGS[1] : nothing)

println("updateVersionDbReturnCode=$(ret.update_needed)")
