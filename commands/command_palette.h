#pragma once

#include <string>
#include <vector>
#include <functional>

namespace isimpa {

struct Command {
    std::string name;
    std::string shortcut;  // Display string like "Ctrl+N"
    std::function<void()> action;
};

class CommandPalette {
public:
    CommandPalette();

    void Open();
    void Close();
    bool IsOpen() const { return m_isOpen; }

    // Draw the palette popup. Call from main loop.
    void Draw();

    void RegisterCommand(const std::string& name, const std::string& shortcut,
                         std::function<void()> action);

private:
    void FilterCommands();

    bool m_isOpen = false;
    bool m_justOpened = false;
    char m_searchBuf[256] = {};
    std::vector<Command> m_commands;
    std::vector<int> m_filtered; // indices into m_commands
    int m_selectedIndex = 0;
};

} // namespace isimpa
