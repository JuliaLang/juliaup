module Juliaup

include("Tar/src/Tar.jl")

import Downloads, .Tar, CodecZlib, JSON

include("versions_database.jl")

function julia_main()
    try
        real_main()
    catch
        Base.invokelatest(Base.display_error, Base.catch_stack())
        return 1
    end
    return 0
end

function get_juliauphome_path()
	# TODO Handle JULIA_DEPOT env var here
	return joinpath(homedir(), ".julia", "juliaup")
end

function get_juliaupconfig_path()
	return joinpath(get_juliauphome_path(), "juliaup.json")
end

function tryparse_full_version(value::AbstractString)
	parts = split(value, '.')
	if length(parts)==3 && !any(i->tryparse(Int, i)===nothing, parts)
		return string(value)
	else
		return nothing
	end
end

function tryparse_channel(value::AbstractString)
	parts = split(value, '.')
	if length(parts)==2 && !any(i->tryparse(Int, i)===nothing, parts)
		return value
	elseif length(parts)==1 && tryparse(Int, value)!==nothing
		return value
	else
		return nothing
	end
end

function try_split_platform(value::AbstractString)
	parts = split(value, '~')
	
	if length(parts)==1
		return (version=value, platform=Int===Int64 ? "x64" : "x86")
	elseif length(parts)==2 && parts[2] in ("x64", "x86")
		return (version=string(parts[1]), platform=string(parts[2]))
	else
		return nothing
	end
end

function get_julia_versions()
	version_db = get_version_db()

	relevant_versions = filter(version_db) do i
		return i[2]["stable"] && any(i[2]["files"]) do j
			return j["os"]=="winnt" && j["kind"]=="archive"
		end
	end

	typed_versions = map(relevant_versions) do i
		return VersionNumber(i[1])
	end

	sort!(typed_versions)

	return typed_versions
end

function getJuliaVersionsThatMatchChannel(channelString)
	parts = split(channelString, '.')

	versionsThatWeCouldUse = VersionNumber[]

	# Collect all the known versions of Julia that exist that match our channel into a vector
	for currVersion in reverse(get_julia_versions())
		if length(parts) == 1 && parts[1] == string(currVersion.major)
			push!(versionsThatWeCouldUse, currVersion)
		elseif length(parts) == 2 && parts[1] == string(currVersion.major) && parts[2] == string(currVersion.minor)
			push!(versionsThatWeCouldUse, currVersion)
		end
	end

	return versionsThatWeCouldUse
end

function install_version(version::String, config_data::Dict{String,Any})
	if haskey(config_data["InstalledVersions"], version)
		return
	else
		version_db = get_version_db()

		download_url = version_db["AvailableVersions"][version]["Url"]

		first_split = try_split_platform(version)

		target_path = joinpath(get_juliauphome_path(), first_split.platform)

		mkpath(target_path)

		println("Installing Julia $(first_split.version) ($(first_split.platform)).")

		temp_file = Downloads.download(download_url)

		try
			open(temp_file) do tar_gz
				tar = CodecZlib.GzipDecompressorStream(tar_gz)
				try
					mktempdir() do extract_temp_path
						Tar.extract(tar, extract_temp_path, same_permissions=false)
						mv(joinpath(extract_temp_path, "julia-$(first_split.version)"), joinpath(target_path, "julia-$(first_split.version)"), force=true)
					end
				finally
					close(tar)
				end
			end

			config_data["InstalledVersions"][version] = Dict("Path" => joinpath(".", first_split.platform, "julia-$(first_split.version)"))

			println("New version successfully installed.")
		finally
			rm(temp_file, force=true)
		end
	end
end

const g_version_db = Ref{Dict}()

function get_version_db()
	if !isassigned(g_version_db)
		version_db_search_paths = [
			joinpath(get_juliauphome_path(), "juliaup-versionsdb-winnt-$(Int===Int64 ? "x64" : "x86").json"),
			joinpath(Sys.BINDIR, "..", "..", "VersionsDB", "juliaup-versionsdb-winnt-$(Int===Int64 ? "x64" : "x86").json") # This only exists when MSIX deployed
		]
		for i in version_db_search_paths
			if isfile(i)
				# TODO Remove again
				println("DEBUG: We are using `", i, "` as the version DB file.")
				g_version_db[] = JSON.parsefile(i, use_mmap=false)
				return g_version_db[]
			end
		end

		error("No version database found.")
	else
		return g_version_db[]
	end
end

function load_config_db()
	juliaup_config_file_path = get_juliaupconfig_path()
	if isfile(juliaup_config_file_path)
		return JSON.parsefile(juliaup_config_file_path, use_mmap=false)
	else
		return Dict{String,Any}("Default"=>"release", "InstalledVersions"=>Dict{String,Any}(), "InstalledChannels"=>Dict{String,Any}())
	end
end

function save_config_db(config_db)
	juliaup_config_file_path = get_juliaupconfig_path()

	open(juliaup_config_file_path, "w") do f
		JSON.print(f, config_db, 4)
	end
end

function is_valid_channel(version_db::Dict{String,Any}, channel::String)
	return haskey(version_db["AvailableChannels"], channel)
end

function get_latest_version_for_channel(channel::String)
	version_db = get_version_db()
	return version_db["AvailableChannels"][channel]["Version"]
end

function garbage_collect_versions(config_data::Dict{String,Any})
	default_channel = config_data["Default"]
	versions_to_uninstall = filter(config_data["InstalledVersions"]) do i
		version = i[1]
		return default_channel!=version && all(config_data["InstalledChannels"]) do j
			if haskey(j[2], "Version")
				return j[2]["Version"] != version
			else
				return true
			end
		end
	end

	for i in versions_to_uninstall
		try
			path_to_delete = joinpath(get_juliauphome_path(), i[2]["Path"])
			rm(path_to_delete, force=true, recursive=true)

			delete!(config_data["InstalledVersions"], i[1])
		catch
			println("WARNING: Failed to delete $path_to_delete.")
		end
	end
end

function update_channel(config_db::Dict{String,Any}, channel::String)
	version_db = get_version_db()

	if version_db["AvailableChannels"][channel]["Version"]!=config_db["InstalledChannels"][channel]["Version"]
		install_version(version_db["AvailableChannels"][channel]["Version"], config_db)

		config_db["InstalledChannels"][channel]["Version"] = version_db["AvailableChannels"][channel]["Version"]
	end
end

function real_main()
    if length(ARGS)==0
        println("Julia Version Manager Preview")
		println()
		println("juliaup command line utility enables configuration of the default Julia version from the command line.")
		println()
		println("usage: juliaup [<command>] [<options>]")
		println()
		println("The following commands are available:")
		println()
		println("  default       Set the default Julia version")
		println("  add           Add a specific Julia version to your system")
		println("  link          Link an existing Julia binary")
		println("  update        Update all or a specific channel to the latest Julia version")
		println("  status        Show all installed Julia versions")
		println("  remove        Remove a Julia version from your system")
		println()
		println("For more details on a specific command, pass it the help argument. [-?] (not yet implemented)")
		println()
		println("The following options are available:")
		println("  -v,--version  Display the version of the tool")
		println("  --info        Display general info of the tool")
		println()
    elseif length(ARGS)>0

		if ARGS[1] == "-v" || ARGS[1] == "--version"
			if length(ARGS)==1
				println("v", JULIA_APP_VERSION)
			else
				println("ERROR: The ", ARGS[1], " argument does not accept any additional arguments.")
			end
		elseif ARGS[1] == "--info"
			if length(ARGS)==1
				println("Julia Version Manager Preview")
				println("Copyright (c) David Anthoff")
			else
				println("ERROR: The --info argument does not accept any additional arguments.")
			end
		elseif ARGS[1] == "default"
			if length(ARGS)==2
				full_channel = ARGS[2]
				if is_valid_channel(get_version_db(), full_channel)
					data = load_config_db()

					data["Default"] = full_channel

					save_config_db(data)

					println("Configured the default Julia version to be ", ARGS[2], ".")
				else
					println("ERROR: '", ARGS[2], "' is not a valid Julia version.")
				end
			else
				println("ERROR: The default command only accepts one additional argument.")
			end
		elseif ARGS[1] == "add"
			if length(ARGS)==2
				full_channel = ARGS[2]

				if is_valid_channel(get_version_db(), full_channel)
					config_data = load_config_db()

					if !haskey(config_data["InstalledChannels"], full_channel)
						required_version = get_latest_version_for_channel(full_channel)

						install_version(required_version, config_data)

						config_data["InstalledChannels"][full_channel] = Dict{String,Any}("Version"=>required_version)

						save_config_db(config_data)
					else
						println("ERROR: '", ARGS[2], "' is already installed.")
					end
				else
					println("ERROR: '", ARGS[2], "' is not a valid Julia version.")
				end
			else
				println("ERROR: The add command only accepts one additional argument.")
			end
		elseif ARGS[1] == "link"
			if length(ARGS)==3
				channel_name = ARGS[2]
				destination_path = ARGS[3]

				config_db = load_config_db()
				version_db = get_version_db()

				if !haskey(config_db["InstalledChannels"], channel_name)
					if haskey(version_db["AvailableChannels"], channel_name)
						println("WARNING: The channel name `$channel_name` is also a system channel. By linking your custom binary to this channel you are hiding this system channel.")
					end

					config_db["InstalledChannels"][channel_name] = Dict{String,Any}("Command"=>destination_path)
				else
					println("ERROR: Channel name `$channel_name` is already used.")
				end

				save_config_db(config_db)
			else
				println("ERROR: The link command only accepts two additional argument.")
			end
		elseif ARGS[1] == "update" || ARGS[1] == "up"
			if length(ARGS)==1
				config_db = load_config_db()
				version_db = get_version_db()

				for i in config_db["InstalledChannels"]
					if haskey(i[2], "Version")
						update_channel(config_db, i[1])
					end
				end

				garbage_collect_versions(config_db)

				save_config_db(config_db)
			elseif length(ARGS)==2
				full_channel = ARGS[2]
				config_db = load_config_db()
				version_db = get_version_db()

				if haskey(config_db["InstalledChannels"], full_channel)
					if haskey(config_db["InstalledChannels"][full_channel], "Version")
						update_channel(config_db, full_channel)

						garbage_collect_versions(config_db)
					else
						println("ERROR: `$full_channel` is a linked channel that cannot be updated.")
					end
				else
					println("Julia $full_channel cannot be updated because it is currently not installed.")
				end

				save_config_db(config_db)
			else
				println("ERROR: The update command accepts at most one additional argument.")
			end
		elseif ARGS[1] == "remove" || ARGS[1] == "rm"			

			if length(ARGS)==2
				full_channel = ARGS[2]
				config_data = load_config_db()

				if haskey(config_data["InstalledChannels"], full_channel)
					if full_channel!=config_data["Default"]
						delete!(config_data["InstalledChannels"], full_channel)

						garbage_collect_versions(config_data)

						save_config_db(config_data)

						println("Julia $full_channel successfully removed.")
					else
						println("ERROR: `$(full_channel)` cannot be removed because it is currently configured as the default channel.")
					end
				else
					println("Julia $full_channel cannot be removed because it is currently not installed.")
				end
			else
				println("ERROR: The remove command only accepts one additional argument.")
			end
		elseif ARGS[1] == "status" || ARGS[1] == "st"
			if length(ARGS)==1
				config_data = load_config_db()

				version_db = get_version_db()

				defaultJulia = config_data["Default"]

				println("Installed Julia channels (default marked with *):")

				for i in config_data["InstalledChannels"]
					if i[1] == defaultJulia
						print("  * ")
					else
						print("    ")
					end
					print("$(i[1])")			

					if haskey(i[2], "Command")
						print(" (linked to `$(i[2]["Command"])`)")
					elseif (version_db["AvailableChannels"][i[1]]["Version"]!=i[2]["Version"])
						print(" (Update from $(i[2]["Version"]) to $(version_db["AvailableChannels"][i[1]]["Version"]) available)")
					end

					println()
				end
			else
				println("ERROR: The status command does not accept any additional arguments.")
			end
		else
			println("ERROR: '", ARGS[1], "' is not a recognized command.")
		end
	else
		println("Internal error.")
	end

    return
end

if abspath(PROGRAM_FILE) == @__FILE__
    real_main()
end

end
