module;

#include <string>
#include <string_view>
#include <vector>
#include <set>
#include <algorithm>

export module Tokenizer;

export template<class T>
void tokenize(std::basic_string<T>& str, T delim, std::vector<std::basic_string<T>>& out)
{
	size_t start;
	size_t end = 0;

	while ((start = str.find_first_not_of(delim, end)) != std::string::npos)
	{
		end = str.find(delim, start);
		out.push_back(str.substr(start, end - start));
	}
}
