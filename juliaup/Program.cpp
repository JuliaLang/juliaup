#include "pch.h"

using namespace winrt;
using namespace Windows::Foundation;
using namespace Windows::ApplicationModel;
using namespace Windows::Storage;

int main()
{
	// init_apartment();

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
		std::cout << "  status        Show all installed Julia versions" << std::endl;
		std::cout << "  remove        Remove a Julia version from your system" << std::endl;
		std::cout << std::endl;
		std::cout << "For more details on a specific command, pass it the help argument. [-?]" << std::endl;
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
				// TODO Extract proper version from somewhere so that it is not hardcoded.
				std::cout << "v1.0.0.0" << std::endl;
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

				// TODO Come up with a less hardcoded version of this.
				if (secondArg == "1.4.2" || secondArg == "1.4.1" || secondArg == "1.4.0") {
					localSettings.Values().Insert(L"version", box_value(winrt::to_hstring(secondArg)));

					std::cout << "Configured the default Julia version to be " << secondArg << "." << std::endl;
				}
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.4.0', '1.4.1' or '1.4.2'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The setdefault command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "add") {
			if (__argc == 3) {
				auto secondArg = std::string(__argv[2]);

				// TODO Come up with a less hardcoded version of this.
				if (secondArg == "1.4.2" || secondArg == "1.4.1" || secondArg == "1.4.0") {

					auto catalog = PackageCatalog::OpenForCurrentPackage();

					auto packageToInstall = std::string("Julia-") + secondArg + "_m018azp39xxy8";

					std::cout << "Installing Julia " << secondArg << "." << std::endl;

					auto res = catalog.AddOptionalPackageAsync(winrt::to_hstring(packageToInstall)).get();

					auto ext_err = res.ExtendedError();

					if (ext_err == 0) {
						std::cout << "New version successfully installed." << std::endl;
					}
					else {
						auto err = hresult_error(ext_err);

						std::wcout << err.message().c_str() << std::endl;
					}					
				}
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.4.0', '1.4.1' or '1.4.2'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The add command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "remove") {
			if (__argc == 3) {
				auto secondArg = std::string(__argv[2]);

				// TODO Come up with a less hardcoded version of this.
				if (secondArg == "1.4.2" || secondArg == "1.4.1" || secondArg == "1.4.0") {
					auto juliaVersionToUninstall = secondArg;

					auto allInstalledDeps = Package::Current().Dependencies();

					bool foundJuliaVersion = false;

					for (auto v : allInstalledDeps) {
						std::wstring name{ v.Id().Name() };

						if (name == L"Julia-" + winrt::to_hstring(juliaVersionToUninstall)) {
							foundJuliaVersion = true;
							break;
						}
					}

					if (foundJuliaVersion) {
						auto packagesToUninstall{ winrt::single_threaded_vector<winrt::hstring>({ winrt::to_hstring(std::string("Julia-") + juliaVersionToUninstall + "_m018azp39xxy8") }) };

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
					}
				}
				else {
					// TODO Come up with a less hardcoded version of this.
					std::cout << "ERROR: '" << secondArg << "' is not a valid Julia version. Valid values are '1.4.0', '1.4.1' or '1.4.2'." << std::endl;
				}
			}
			else {
				std::cout << "ERROR: The remove command only accepts one additional argument." << std::endl;
			}
		}
		else if (firstArg == "status") {
			if (__argc == 2) {
				std::cout << "The following Julia versions are currently installed:" << std::endl;

				auto allInstalledDeps = Package::Current().Dependencies();

				for (auto v : allInstalledDeps) {
					std::wstring name{ v.Id().Name() };

					if (name.starts_with(L"Julia-")) {
						std::wcout << L"  " << name << std::endl;
					}
				}

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
