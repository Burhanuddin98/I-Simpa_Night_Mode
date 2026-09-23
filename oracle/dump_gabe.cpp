// Canonical dump of a GABE table (docs/formats/gabe.md) through upstream's own reader,
// formatGABE::GABE::Load (lib_interface/input_output/gabe/gabe.cpp), which build.ps1 already
// compiles in. Everything printed comes from the loaded GABE object, except the version: GABE
// keeps no copy of it, so it is re-read from the file's first 4 bytes, the field Load gated on.
#include <cstdint>
#include <cstdio>
#include <cstring>

#include "common.hpp"
#include "input_output/gabe/gabe.h"

using namespace formatGABE;

// Bytes up to the first NUL, at most n: upstream's label and cell fields need not be terminated.
static std::string field(const char* s, std::size_t n) { return str_token(s, strnlen(s, n)); }

int dump_gabe(const char* path) {
    GABE table;
    if (!table.Load(path)) return oracle_fail();

    std::int32_t version = 0;
    FILE* f = std::fopen(path, "rb");
    if (!f) return oracle_fail();
    std::size_t got = std::fread(&version, sizeof version, 1, f);
    std::fclose(f);
    if (got != 1) return oracle_fail();

    std::printf("gabe %d\n", static_cast<int>(version));
    std::printf("readonly %d\n", table.IsReadOnly() ? 1 : 0);
    std::printf("columns %ld\n", static_cast<long>(table.GetCols()));
    for (Longb i = 0; i < table.GetCols(); ++i) {
        GABE_Object* col = table.GetCol(i);
        if (!col) return oracle_fail();
        const std::string label = field(col->GetLabel(), STRING_LABEL_LENGTH);
        GABE_Data_Float* fcol = nullptr;
        GABE_Data_Integer* icol = nullptr;
        GABE_Data_ShortString* scol = nullptr;
        if (table.GetCol(i, &fcol)) {
            std::printf("column float %s\n", label.c_str());
            std::printf("digits %d\n", static_cast<int>(fcol->headerData.numOfDigits));
            std::printf("rows %lu\n", static_cast<unsigned long>(fcol->GetSize()));
            for (ULongb r = 0; r < fcol->GetSize(); ++r)
                std::printf("%s\n", f32_hex(fcol->GetValue(r)).c_str());
        } else if (table.GetCol(i, &icol)) {
            std::printf("column int %s\n", label.c_str());
            std::printf("rows %lu\n", static_cast<unsigned long>(icol->GetSize()));
            for (ULongb r = 0; r < icol->GetSize(); ++r)
                std::printf("%d\n", static_cast<int>(icol->GetValue(r)));
        } else if (table.GetCol(i, &scol)) {
            std::printf("column shortstring %s\n", label.c_str());
            std::printf("rows %lu\n", static_cast<unsigned long>(scol->GetSize()));
            for (ULongb r = 0; r < scol->GetSize(); ++r)
                std::printf("%s\n", field(scol->GetValue(r).strData, sizeof(t_StringShort)).c_str());
        } else {
            return oracle_fail();  // Load keeps only these three types
        }
    }
    return 0;
}
