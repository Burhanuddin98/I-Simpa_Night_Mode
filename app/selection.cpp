#include "app/selection.h"

namespace isimpa {
static Selection g_selection;
Selection& GetSelection() { return g_selection; }
}
