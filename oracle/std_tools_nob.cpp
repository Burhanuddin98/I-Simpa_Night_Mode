// Boost-free replacement for upstream's std_tools.cpp (std::filesystem), used only by the oracle.
// From the 2026-09-23 contract survey: the format layer of lib_interface needs nothing else from Boost.
#include "std_tools.hpp"
#include <cmath>
#include <filesystem>
bool st_mkdir(const std::string& p) { std::error_code ec; return std::filesystem::create_directories(std::filesystem::u8path(p), ec); }
bool st_isfinite(const float& v) { return std::isfinite(v); }
std::string st_path_separator() { return std::string(1, (char)std::filesystem::path::preferred_separator); }
