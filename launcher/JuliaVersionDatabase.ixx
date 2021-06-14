module;

#include <string>
#include <string_view>
#include <codecvt>
#include <vector>
#include <set>
#include <ranges>
#include <algorithm>
#include <filesystem>
#include <iostream>
#include <fstream>
#include "../json/single_include/nlohmann/json.hpp"

export module JuliaVersionDatabase;

import Tokenizer;

using json = nlohmann::json;

struct JuliaVersion
{
	int major;
	int minor;
	int patch;

	JuliaVersion(int a, int b, int c) : major(a), minor(b), patch(c) {}

	JuliaVersion(std::string value)
	{
		std::vector<std::string> parts;
		tokenize(value, '.', parts);

		// TODO Check for invalid values
		major = std::stoi(parts[0]);
		minor = std::stoi(parts[1]);
		patch = std::stoi(parts[2]);
	}

	bool operator<(const JuliaVersion& b) const
	{
		if (major == b.major) {
			if (minor == b.minor) {
				return patch < b.patch;
			}
			else {
				return minor < b.minor;
			}
		}
		else {
			return major < b.major;
		}
	}

public:
	std::string toString() {
		return std::string(std::to_string(this->major) + "." + std::to_string(this->minor) + "." + std::to_string(this->patch));
	}
};

export class JuliaVersionsDatabase
{
private:
	std::vector<JuliaVersion> m_juliaVersions;

public:
	//std::vector<JuliaVersion> getHardcodedJuliaVersions();

	//std::wstring getBundledJuliaVersion();

	void init(std::filesystem::path juliaupPath) {
		auto versionsFilePath = juliaupPath / "versions.json";

		if (std::filesystem::exists(versionsFilePath)) {
			std::ifstream i(versionsFilePath);

			json versionsData;

			i >> versionsData;

			for (auto& [key, value] : versionsData.items())
			{
				if (value["stable"].get<bool>() == true)
				{
					m_juliaVersions.push_back(JuliaVersion{ key });
				}
			}
		}
		else
		{
			getHardcodedJuliaVersions();
		}

		std::sort(m_juliaVersions.begin(), m_juliaVersions.end());
	}

	std::vector<JuliaVersion> getJuliaVersions() {
		return m_juliaVersions;
	}

	bool isValidJuliaVersion(std::string_view versionString) {
		auto versions = this->getJuliaVersions();

		return std::any_of(versions.begin(), versions.end(), [&](auto i) {return versionString == i.toString() || versionString == i.toString() + "-x86"; });
	}

	bool isValidJuliaChannel(std::string_view versionString)
	{
		auto versions = this->getJuliaVersions();

		std::set<std::string> channels;

		for (auto const& i : versions) {
			channels.insert(std::to_string(i.major));
			channels.insert(std::to_string(i.major) + "." + std::to_string(i.minor));

			channels.insert(std::to_string(i.major) + "-x86");
			channels.insert(std::to_string(i.major) + "." + std::to_string(i.minor) + "-x86");
		}

		return std::any_of(channels.begin(), channels.end(), [&](auto i) {return i == versionString; });
	}

	std::vector<std::string> getJuliaVersionsThatMatchChannel(std::string channelString) {
		std::vector<std::string> parts;
		tokenize(channelString, '.', parts);

		std::vector<std::string> versionsThatWeCouldUse;

		auto juliaVersions = this->getJuliaVersions();

		// Collect all the known versions of Julia that exist that match our channel into a vector
		;
		for (auto& currVersion : std::ranges::reverse_view{ juliaVersions }) {
			if (parts.size() == 1 && parts[0] == std::to_string(currVersion.major)) {
				versionsThatWeCouldUse.push_back(currVersion.toString());
			}
			else if (parts.size() == 2 && parts[0] == std::to_string(currVersion.major) && parts[1] == std::to_string(currVersion.minor)) {
				versionsThatWeCouldUse.push_back(currVersion.toString());
			}
		}

		return versionsThatWeCouldUse;
	}

	std::vector<JuliaVersion> getHardcodedJuliaVersions() {
		std::vector<JuliaVersion> juliaVersions = {
		JuliaVersion{1, 6, 0}, JuliaVersion{1, 6, 1}
		};
		return juliaVersions;
	}

	std::wstring getBundledJuliaVersion() {
		return std::wstring{ L"1.6.1" };
	}
};
