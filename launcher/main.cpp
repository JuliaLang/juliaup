#include "pch.h"

using namespace winrt;
using namespace Windows::ApplicationModel;
using namespace Windows::Storage;

void tokenize(std::wstring& str, char delim, std::vector<std::wstring>& out)
{
	size_t start;
	size_t end = 0;

	while ((start = str.find_first_not_of(delim, end)) != std::wstring::npos)
	{
		end = str.find(delim, start);
		out.push_back(str.substr(start, end - start));
	}
}

std::string GetLastErrorAsString()
{
	//Get the error message, if any.
	DWORD errorMessageID = ::GetLastError();
	if (errorMessageID == 0)
		return std::string(); //No error message has been recorded

	LPSTR messageBuffer = nullptr;
	size_t size = FormatMessageA(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
		NULL, errorMessageID, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), (LPSTR)&messageBuffer, 0, NULL);

	std::string message(messageBuffer, size);

	//Free the buffer.
	LocalFree(messageBuffer);

	return message;
}

HRESULT StartProcess(LPCWSTR applicationName, LPWSTR commandLine, LPCWSTR currentDirectory, DWORD timeout)
{
	STARTUPINFO info;
	GetStartupInfo(&info);

	PROCESS_INFORMATION processInfo{};

	BOOL ret = CreateProcessW(
		applicationName,
		GetCommandLineW(), //commandLine,
		nullptr, nullptr, // Process/ThreadAttributes
		true, // InheritHandles
		0, //EXTENDED_STARTUPINFO_PRESENT, // CreationFlags
		nullptr, // Environment
		nullptr, //currentDirectory,
		//(LPSTARTUPINFO)&startupInfoEx,
		&info,
		&processInfo);

	if (!ret) {
		auto error_message = GetLastErrorAsString();

		printf(error_message.c_str());

		return ERROR;
	}

	// RETURN_HR_IF(HRESULT_FROM_WIN32(ERROR_INVALID_HANDLE), processInfo.hProcess == INVALID_HANDLE_VALUE);
	DWORD waitResult = ::WaitForSingleObject(processInfo.hProcess, timeout);
	// RETURN_LAST_ERROR_IF_MSG(waitResult != WAIT_OBJECT_0, "Waiting operation failed unexpectedly.");
	CloseHandle(processInfo.hProcess);
	CloseHandle(processInfo.hThread);

	return ERROR_SUCCESS;
}

int wmain(int argc, wchar_t* argv[], wchar_t* envp[]) {
	init_apartment();

	SetConsoleTitle(L"Julia");

	auto juliaVersionsDatabase = new JuliaVersionsDatabase();

	auto localSettings = ApplicationData::Current().LocalSettings();

	std::wstring juliaVersionToUse = L"1";

	if (localSettings.Values().HasKey(L"version")) {
		juliaVersionToUse = unbox_value<winrt::hstring>(localSettings.Values().Lookup(L"version"));
	}

	std::vector<std::wstring> parts;
	tokenize(juliaVersionToUse, L'-', parts);
	auto &versionPart = parts[0];
	auto platformPart = parts.size() > 1 ? parts[1] : L"";

	// Now figure out whether we got a channel or a specific version.
	std::vector<std::wstring> parts2;
	tokenize(versionPart, L'.', parts2);

	

	winrt::hstring julia_path;

	// We are using a specific Julia version
	if (parts2.size() == 3) {
		std::wstring formattedJuliaVersionToUse = L"";
		formattedJuliaVersionToUse = parts.size() == 1 ? versionPart : platformPart + L"-" + versionPart;

		auto allInstalledDeps = Package::Current().Dependencies();

		bool foundJuliaVersion = false;

		for (auto v : allInstalledDeps) {
			std::wstring name{ v.Id().Name() };

			if (name == L"Julia-" + formattedJuliaVersionToUse) {
				auto juliaBinaryStorageLocation = v.InstalledLocation().GetFileAsync(L"Julia\\bin\\julia.exe").get();
				julia_path = juliaBinaryStorageLocation.Path();
				foundJuliaVersion = true;
				SetConsoleTitle((L"Julia " + formattedJuliaVersionToUse).c_str());
				break;
			}
		}

		if (!foundJuliaVersion) {
			std::wcout << L"Julia version " + juliaVersionToUse + L" is not installed on this system. Run:" << std::endl;
			std::wcout << std::endl;
			std::wcout << L"  juliaup add " + juliaVersionToUse << std::endl;
			std::wcout << std::endl;
			std::wcout << L"to install it." << std::endl;

			return 1;
		}
	}
	// We are using a channel
	else {
		std::wstring formattedJuliaVersionToUse = L"";
		std::vector<std::wstring> versionsThatWeCouldUse;

		auto juliaVersions = juliaVersionsDatabase->getJuliaVersions();

		// Collect all the known versions of Julia that exist that match our channel into a vector
		for (int i = juliaVersions.size() - 1; i >= 0; i--) {
			auto& currVersion = juliaVersions[i];
			if (parts2.size() == 1 && parts2[0] == std::to_wstring(currVersion.major)) {
				auto as_string = currVersion.toString();
				versionsThatWeCouldUse.push_back(std::wstring(as_string.begin(), as_string.end()));
			}
			else if (parts2.size() == 2 && parts2[0] == std::to_wstring(currVersion.major) && parts2[1] == std::to_wstring(currVersion.minor)) {
				auto as_string = currVersion.toString();
				versionsThatWeCouldUse.push_back(std::wstring(as_string.begin(), as_string.end()));
			}
		}

		if (versionsThatWeCouldUse.size() > 0) {
			auto allInstalledDeps = Package::Current().Dependencies();

			bool foundLatestJuliaVersionForChannel = false;
			bool foundAnyJuliaVersionForChannel = false;
			
			for (int i = 0; i < versionsThatWeCouldUse.size(); i++) {
				formattedJuliaVersionToUse = parts.size() == 1 ? versionsThatWeCouldUse[i] : platformPart + L"-" + versionsThatWeCouldUse[i];

				for (auto v : allInstalledDeps) {
					std::wstring name{ v.Id().Name() };

					if (name == L"Julia-" + formattedJuliaVersionToUse) {
						auto juliaBinaryStorageLocation = v.InstalledLocation().GetFileAsync(L"Julia\\bin\\julia.exe").get();
						julia_path = juliaBinaryStorageLocation.Path();
						foundLatestJuliaVersionForChannel = i==0;
						foundAnyJuliaVersionForChannel = true;
						SetConsoleTitle((L"Julia " + formattedJuliaVersionToUse).c_str());
						break;
					}
				}
				if(foundAnyJuliaVersionForChannel)
					break;
			}

			if (!foundAnyJuliaVersionForChannel) {
				std::wcout << L"No Julia version for channel " + juliaVersionToUse + L" is installed on this system. Run:" << std::endl;
				std::wcout << std::endl;
				std::wcout << L"  juliaup update" << std::endl;
				std::wcout << std::endl;
				std::wcout << L"to install Julia " << juliaVersionToUse << ", the latest Julia version for the current channel." << std::endl;

				return 1;
			}

			if (!foundLatestJuliaVersionForChannel) {
				std::wcout << L"The latest version of Julia in the " << juliaVersionToUse << " channel is Julia " << versionsThatWeCouldUse[0] << ". You currently have Julia " << formattedJuliaVersionToUse << " installed. Run:" << std::endl;
				std::wcout << std::endl;
				std::wcout << L"  juliaup update " << std::endl;
				std::wcout << std::endl;
				std::wcout << L"to install Julia " << versionsThatWeCouldUse[0] << "." << std::endl;
			}
		}
		else {
			std::wcout << L"No Julia versions for channel " + juliaVersionToUse + L" exist. Please select a different channel." << std::endl;
			return 1;
		}
	}

	std::filesystem::path exePath = std::wstring{ julia_path };
	std::filesystem::path currentDirectory = L"";
	std::wstring exeArgString = (wchar_t*)L"";

	std::wstring fullargs = (L"\"" + exePath.native() + L"\" " + exeArgString + L" "); // +args);

	fullargs = L"";
	HRESULT hr = StartProcess(exePath.c_str(), fullargs.data(), currentDirectory.c_str(), INFINITE);
	if (hr != ERROR_SUCCESS)
	{
		printf("Error return from launching process.");
	}

	return 0;
}
