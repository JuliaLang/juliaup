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

extern "C" IMAGE_DOS_HEADER __ImageBase;

std::wstring GetExecutablePath()
{
	std::wstring buffer;
	size_t nextBufferLength = MAX_PATH;

	for (;;)
	{
		buffer.resize(nextBufferLength);
		nextBufferLength *= 2;

		SetLastError(ERROR_SUCCESS);

		auto pathLength = GetModuleFileName(reinterpret_cast<HMODULE>(&__ImageBase), &buffer[0], static_cast<DWORD>(buffer.length()));

		if (pathLength == 0)
			throw std::exception("GetModuleFileName failed"); // You can call GetLastError() to get more info here

		if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
		{
			buffer.resize(pathLength);
			return buffer;
		}
	}
}

std::wstring getCurrentPlatform() {
#ifdef _M_X64
	return std::wstring{ L"x64" };
#endif

#ifdef _M_IX86
	return std::wstring{ L"x86" };
#endif
}

std::filesystem::path getJuliaupPath() {
	std::filesystem::path homedirPath = std::wstring{ Windows::Storage::UserDataPaths::GetDefault().Profile() };
	return homedirPath / ".julia" / "juliaup";
}

std::wstring s2ws(const std::string& str)
{
	using convert_typeX = std::codecvt_utf8<wchar_t>;
	std::wstring_convert<convert_typeX, wchar_t> converterX;

	return converterX.from_bytes(str);
}

std::string ws2s(const std::wstring& wstr)
{
	using convert_typeX = std::codecvt_utf8<wchar_t>;
	std::wstring_convert<convert_typeX, wchar_t> converterX;

	return converterX.to_bytes(wstr);
}

void initial_setup() {
	auto juliaupFolder = getJuliaupPath();

	if (!std::filesystem::exists(juliaupFolder)) {

		std::filesystem::path myOwnPath = GetExecutablePath();

		auto pathOfBundledJulia = myOwnPath.parent_path().parent_path() / "BundledJulia";

		auto juliaVersionsDatabase = new JuliaVersionsDatabase();

		auto platform = getCurrentPlatform();

		auto targetPath = juliaupFolder / platform / (L"julia-" + juliaVersionsDatabase->getBundledJuliaVersion());

		std::filesystem::create_directories(targetPath);

		std::filesystem::copy(pathOfBundledJulia, targetPath, std::filesystem::copy_options::overwrite_existing | std::filesystem::copy_options::recursive);
	}
}

int wmain(int argc, wchar_t* argv[], wchar_t* envp[]) {
	init_apartment();

	SetConsoleTitle(L"Julia");

	initial_setup();

	auto juliaVersionsDatabase = new JuliaVersionsDatabase();

	std::wstring juliaVersionToUse = L"1";

	auto configFilePath = getJuliaupPath() / "juliaup.toml";

	if (std::filesystem::exists(configFilePath)) {
		try {
			auto data = toml::parse(configFilePath);

			auto value_as_string = toml::find<std::string>(data, "currentversion");

			juliaVersionToUse = std::wstring_convert<std::codecvt_utf8<wchar_t>>().from_bytes(value_as_string);
		}
		catch (...) {
			std::wcout << "Could not read the juliaup configuration file, using the default value of '1' as the version to use." << std::endl;
		}
	}

	std::vector<std::wstring> parts;
	tokenize(juliaVersionToUse, L'-', parts);
	auto &versionPart = parts[0];
	auto platformPart = parts.size() > 1 ? parts[1] : getCurrentPlatform();

	// Now figure out whether we got a channel or a specific version.
	std::vector<std::wstring> parts2;
	tokenize(versionPart, L'.', parts2);

	std::filesystem::path julia_path;

	// We are using a specific Julia version
	if (parts2.size() == 3) {
		auto targetPath = getJuliaupPath() / platformPart / (L"julia-" + versionPart);

		if (std::filesystem::exists(targetPath)) {
			julia_path = targetPath / L"bin" / L"julia.exe";
			SetConsoleTitle((L"Julia " + versionPart).c_str());
		}
		else {
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
			bool foundLatestJuliaVersionForChannel = false;
			bool foundAnyJuliaVersionForChannel = false;
			std::wstring juliaVersionWeAreUsing;
			
			for (int i = 0; i < versionsThatWeCouldUse.size(); i++) {
				auto targetPath = getJuliaupPath() / platformPart / (L"julia-" + versionsThatWeCouldUse[i]);

				if (std::filesystem::exists(targetPath)) {
					julia_path = targetPath / L"bin" / L"julia.exe";
					foundLatestJuliaVersionForChannel = i == 0;
					foundAnyJuliaVersionForChannel = true;
					juliaVersionWeAreUsing = versionsThatWeCouldUse[i];
					SetConsoleTitle((L"Julia " + versionsThatWeCouldUse[i]).c_str());
					break;
				}
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
				std::wcout << L"The latest version of Julia in the " << juliaVersionToUse << " channel is Julia " << versionsThatWeCouldUse[0] << ". You currently have Julia " << juliaVersionWeAreUsing << " installed. Run:" << std::endl;
				std::wcout << std::endl;
				std::wcout << L"  juliaup update " << std::endl;
				std::wcout << std::endl;
				std::wcout << L"to install Julia " << versionsThatWeCouldUse[0] << " and update your current channel to that version." << std::endl;
			}
		}
		else {
			std::wcout << L"No Julia versions for channel " + juliaVersionToUse + L" exist. Please select a different channel." << std::endl;
			return 1;
		}
	}

	std::filesystem::path currentDirectory = L"";
	std::wstring exeArgString = (wchar_t*)L"";

	std::wstring fullargs = (L"\"" + julia_path.native() + L"\" " + exeArgString + L" "); // +args);

	fullargs = L"";
	HRESULT hr = StartProcess(julia_path.c_str(), fullargs.data(), currentDirectory.c_str(), INFINITE);
	if (hr != ERROR_SUCCESS)
	{
		printf("Error return from launching process.");
	}

	return 0;
}
