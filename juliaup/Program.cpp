#include "pch.h"

using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::ApplicationModel;
using namespace Windows::Storage;

void tokenize(std::string& str, char delim, std::vector<std::string>& out)
{
	size_t start;
	size_t end = 0;

	while ((start = str.find_first_not_of(delim, end)) != std::string::npos)
	{
		end = str.find(delim, start);
		out.push_back(str.substr(start, end - start));
	}
}

int main()
{
	// init_apartment();

	auto juliaVersions = new JuliaVersionsDatabase();

	auto localSettings = ApplicationData::Current().LocalSettings();

	if (__argc == 1) {
		std::cout << "Julia Version Manager Preview" << std::endl;
		std::cout << std::endl;
		std::cout << "juliaup command line utility enables configuration of the default Julia version from the command line." << std::endl;
		std::cout << std::endl;
		std::cout << "usage: juliaup [<command>] [<options>]" << std::endl;
		std::cout << std::endl;
		std::cout << "The following commands are available:" << std::endl;
		std::cout << std::endl;
		std::cout << "  setdefault    Set the default Julia version" << std::endl;
		std::cout << "  add           Add a specific Julia version to your system" << std::endl;
		std::cout << "  update        Update the current channel to the latest Julia version" << std::endl;
		std::cout << "  status        Show all installed Julia versions" << std::endl;
		std::cout << "  remove        Remove a Julia version from your system" << std::endl;
		std::cout << std::endl;
		std::cout << "For more details on a specific command, pass it the help argument. [-?] (not yet implemented)" << std::endl;
		std::cout << std::endl;
		std::cout << "The following options are available:" << std::endl;
		std::cout << "  -v,--version  Display the version of the tool" << std::endl;
		std::cout << "  --info        Display general info of the tool" << std::endl;
		std::cout << std::endl;
	}
	else if (__argc > 1) {
		auto firstArg = std::string_view(__argv[1]);

		if (firstArg == "-v" || firstArg == "--version") {
			if (__argc == 2) {
				std::cout << "v" << JULIA_APP_VERSION_MAJOR << "." << JULIA_APP_VERSION_MINOR << "." << JULIA_APP_VERSION_REVISION  << "." << JULIA_APP_VERSION_BUILD << std::endl;
			}
			else {
				std::cout << "ERROR: The " << firstArg << " argument does not accept any additional arguments." << std::endl;
			}
		}
		else if (firstArg == "--info") {
			if (__argc == 2) {
				std::cout << "Julia Version Manager Preview (UWP)" << std::endl;
				std::cout << "Copyright (c) David Anthoff" << std::endl;
			}
			else {
				std::cout << "ERROR: The --info argument does not accept any additional arguments." << std::endl;
			}
		}
		else if (firstArg == "setdefault") {
			if (__argc == 3) {
				auto secondArg = std::string_view{ __argv[2] };

				if (juliaVersions->isValidJuliaVersion(secondArg) || juliaVersions->isValidJuliaChannel(secondArg)) {
					localSettings.Values().Insert(L"version", box_value(winrt::to_hstring(secondArg)));

					std::cout << "Configured the default Julia version to be " << secondArg << "." << std::endl;
				}				
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The setdefault command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "add") {
			if (__argc == 3) {
				auto secondArg = std::string(__argv[2]);

				if (juliaVersions->isValidJuliaVersion(secondArg)) {

					/*auto catalog = PackageCatalog::OpenForCurrentPackage();

					std::vector<std::string> parts;
					tokenize(secondArg, '-', parts);
					
					auto packageToInstall = std::string("Julia-") + (parts.size()==2 ? (parts[1] + "-" + parts[0]) : secondArg) + "_b0ra4bp6jsp6c";

					std::cout << "Installing Julia " << secondArg << "." << std::endl;

					auto res = catalog.AddOptionalPackageAsync(winrt::to_hstring(packageToInstall)).get();

					auto ext_err = res.ExtendedError();

					if (ext_err == 0) {
						std::cout << "New version successfully installed." << std::endl;
					}
					else {
						auto err = hresult_error(ext_err);

						std::wcout << err.message().c_str() << std::endl;
					}*/
				}
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The add command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "update" || firstArg == "up") {
			if (__argc == 2) {
				//std::string juliaVersionToUse = "1";

				//if (localSettings.Values().HasKey(L"version")) {
				//	juliaVersionToUse = to_string(unbox_value<winrt::hstring>(localSettings.Values().Lookup(L"version")));
				//}

				//std::vector<std::string> parts;
				//tokenize(juliaVersionToUse, '-', parts);
				//auto& versionPart = parts[0];
				//auto platformPart = parts.size() > 1 ? parts[1] : "";

				//// Now figure out whether we got a channel or a specific version.
				//std::vector<std::string> parts2;
				//tokenize(versionPart, '.', parts2);

				//if (parts2.size() < 3) {
				//	auto publishedVersionsWeCouldUse = juliaVersions->getJuliaVersionsThatMatchChannel(versionPart);

				//	if (publishedVersionsWeCouldUse.size() > 0) {
				//		auto catalog = PackageCatalog::OpenForCurrentPackage();

				//		auto fullVersionString = (parts.size() == 2 ? (parts[1] + "-" + publishedVersionsWeCouldUse[0]) : publishedVersionsWeCouldUse[0]);
				//		auto fullVersionStringNice = (parts.size() == 2 ? (publishedVersionsWeCouldUse[0] + "-" + parts[1]) : publishedVersionsWeCouldUse[0]);

				//		auto packageToInstall = std::string("Julia-") + fullVersionString + "_b0ra4bp6jsp6c";

				//		std::cout << "Installing Julia " << fullVersionStringNice << "." << std::endl;

				//		auto res = catalog.AddOptionalPackageAsync(winrt::to_hstring(packageToInstall)).get();

				//		auto ext_err = res.ExtendedError();

				//		if (ext_err == 0) {
				//			std::cout << "New version successfully installed." << std::endl;
				//		}
				//		else {
				//			auto err = hresult_error(ext_err);

				//			std::wcout << err.message().c_str() << std::endl;
				//		}
				//	}
				//	else {
				//		std::cout << "You currently have a Julia channel configured for which no Julia versions exists. Nothing can be updated." << std::endl;
				//	}
				//}
				//else {
				//	std::cout << "You currently have a specific Julia version as your default configured. Only channel defaults can be updated." << std::endl;
				//}
			}
			else {
				std::cout << "ERROR: The update command does not accept any additional arguments." << std::endl;
			}
		}
		else if (firstArg == "remove" || firstArg == "rm") {
			if (__argc == 3) {
				auto secondArg = std::string(__argv[2]);

				if (juliaVersions->isValidJuliaVersion(secondArg)) {
					/*auto juliaVersionToUninstall = secondArg;

					std::vector<std::string> parts;
					tokenize(juliaVersionToUninstall, L'-', parts);

					auto formattedJuliaVersionToUninstall = parts.size() == 1 ? parts[0] : parts[1] + "-" + parts[0];

					auto allInstalledDeps = Package::Current().Dependencies();

					bool foundJuliaVersion = false;

					for (auto v : allInstalledDeps) {
						std::wstring name{ v.Id().Name() };

						if (name == L"Julia-" + winrt::to_hstring(formattedJuliaVersionToUninstall)) {
							foundJuliaVersion = true;
							break;
						}
					}

					if (foundJuliaVersion) {
						auto packagesToUninstall{ winrt::single_threaded_vector<winrt::hstring>({ winrt::to_hstring(std::string("Julia-") + formattedJuliaVersionToUninstall + "_b0ra4bp6jsp6c") }) };

						auto catalog = PackageCatalog::OpenForCurrentPackage();

						auto res = catalog.RemoveOptionalPackagesAsync(packagesToUninstall).get();

						auto ext_err = res.ExtendedError();

						if (ext_err == NULL) {
							std::cout << "Julia " + juliaVersionToUninstall + " successfully removed." << std::endl;
						}
						else {
							auto err = hresult_error(ext_err);

							std::wcout << err.message().c_str() << std::endl;
						}
					}
					else {
						std::cout << "Julia " + juliaVersionToUninstall  + " cannot be removed because it is currently not installed." << std::endl;
					}*/
				}
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.5.1', '1.5.2', '1.5.3', '1.5.4', '1.6.0' or '1.6.1'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The remove command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "status" || firstArg == "st") {
			if (__argc == 2) {
				std::cout << "The following Julia versions are currently installed:" << std::endl;

				/*auto allInstalledDeps = Package::Current().Dependencies();

				for (auto v : allInstalledDeps) {
					std::wstring name{ v.Id().Name() };

					if (name.starts_with(L"Julia-")) {
						std::wcout << L"  " << name << std::endl;
					}
				}*/

			}
			else {
				std::cout << "ERROR: The status command does not accept any additional arguments." << std::endl;
			}
		}
		else {
			std::cout << "ERROR: '" << firstArg << "' is not a recognized command." << std::endl;
		}
	}
	else {
		std::cout << "Internal error." << std::endl;
	}

	return 0;
}
