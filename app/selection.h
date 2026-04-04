#pragma once

#include <set>

namespace isimpa {

struct Selection {
    int group = -1;
    int source = -1;
    int receiver = -1;

    // Multi-select support for groups
    std::set<int> selectedGroups;

    void SelectGroup(int i) {
        group = i; source = -1; receiver = -1;
        selectedGroups.clear();
        if (i >= 0) selectedGroups.insert(i);
    }
    void ToggleGroup(int i) {
        source = -1; receiver = -1;
        if (selectedGroups.count(i)) {
            selectedGroups.erase(i);
            group = selectedGroups.empty() ? -1 : *selectedGroups.rbegin();
        } else {
            selectedGroups.insert(i);
            group = i;
        }
    }
    bool IsGroupSelected(int i) const {
        return selectedGroups.count(i) > 0;
    }
    void SelectSource(int i)   { source = i; group = -1; receiver = -1; selectedGroups.clear(); }
    void SelectReceiver(int i) { receiver = i; group = -1; source = -1; selectedGroups.clear(); }
    void Clear()               { group = source = receiver = -1; selectedGroups.clear(); }
};

Selection& GetSelection();

} // namespace isimpa
