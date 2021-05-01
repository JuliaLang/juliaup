#include "pch.h"

std::vector<JuliaVersion> JuliaVersionsDatabase::getJuliaVersions() {
	std::vector<JuliaVersion> juliaVersions = { 
    JuliaVersion{1, 5, 1}, JuliaVersion{1, 5, 2}, JuliaVersion{1, 5, 3}, JuliaVersion{1, 5, 4}, JuliaVersion{1, 6, 0}, JuliaVersion{1, 6, 1}
	};
	return juliaVersions;
}
