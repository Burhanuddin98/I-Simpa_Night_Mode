#include "app/app.h"
#include "viewport/viewport.h"
#include <string>
#include <vector>

#ifdef _WIN32
#include <windows.h>
#include <shellapi.h>

// Force NVIDIA discrete GPU on hybrid graphics laptops
extern "C" {
    __declspec(dllexport) unsigned long NvOptimusEnablement = 0x00000001;
    __declspec(dllexport) int AmdPowerXpressRequestHighPerformance = 1;
}

// Parse command line into argv-style vector
static std::vector<std::string> ParseCommandLine() {
    std::vector<std::string> args;
    int argc = 0;
    LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (argv) {
        for (int i = 1; i < argc; i++) { // skip exe name
            int len = WideCharToMultiByte(CP_UTF8, 0, argv[i], -1, nullptr, 0, nullptr, nullptr);
            std::string s(len - 1, 0);
            WideCharToMultiByte(CP_UTF8, 0, argv[i], -1, &s[0], len, nullptr, nullptr);
            args.push_back(s);
        }
        LocalFree(argv);
    }
    return args;
}

int WINAPI WinMain(HINSTANCE, HINSTANCE, LPSTR, int) {
#else
static std::vector<std::string> ParseCommandLine(int argc, char* argv[]) {
    std::vector<std::string> args;
    for (int i = 1; i < argc; i++) args.push_back(argv[i]);
    return args;
}
int main(int argc, char* argv[]) {
#endif

    auto cmdArgs =
#ifdef _WIN32
        ParseCommandLine();
#else
        ParseCommandLine(argc, argv);
#endif

    isimpa::App app;

    if (!app.Init()) {
        return 1;
    }

    // Pass command-line args to the app for queued automation
    app.QueueAutomation(cmdArgs);

    isimpa::InitViewport();
    app.Run();
    isimpa::ShutdownViewport();

    return 0;
}
