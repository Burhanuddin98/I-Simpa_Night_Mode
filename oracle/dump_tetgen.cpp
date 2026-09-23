// Canonical dump of a TetGen .node, .ele, .face or .neigh file (and the _skipped.node /
// _skipped.face pair TetGen writes when a run fails), on TetGen's own tetgenio loaders.
// Spec and dump grammar: docs/formats/tetgen.md.
// oracle-extra-source: tetgen/tetgen.cxx
// oracle-extra-source: tetgen/predicates.cxx
// oracle-define: TETLIBRARY
//
// What is upstream and what is ours:
// - .node, .ele and .face go through tetgenio::load_node, load_tet and load_face unchanged.
// - .neigh has no loader in TetGen. It is read here with tetgenio's own tokenizer
//   (readnumberline, findnextnumber, strtol), in load_tet's shape. This is the weakest leg of
//   the oracle: it checks the Rust tokenizer against TetGen's, not a second reading of the layout.
// - Each file is read on its own. load_tet and load_face check corners against the .node's
//   count and first index; without a .node the oracle presets firstnumber from the file's own
//   first record index (0 or 1, as load_node_call does for points) and numberofpoints to
//   INT_MAX - 1, the largest count for which TetGen's check cannot overflow.
// - Guards, for inputs upstream does not survive: fewer number lines than the header declares
//   (every loader dereferences readnumberline's NULL at end of file), no header at all
//   (load_node parses an unfilled buffer), and a negative .node count (load_node would call
//   new REAL[negative * 3]). The guards count lines with tetgenio::readnumberline itself, so
//   they see the file exactly as the loaders do.
// - TetGen prints progress on stdout; it is sent to NUL while a loader runs.
#include <climits>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <io.h>
#include <string>
#include <vector>

#include "../tetgen/tetgen.h"
#include "common.hpp"

namespace {

enum Kind { K_NODE, K_ELE, K_FACE, K_NEIGH, K_UNKNOWN };

// Kind from the extension (ASCII case-insensitive, as Windows opens it); base is the path
// without the extension, which is what the tetgenio loaders take.
Kind kind_of(const std::string& path, std::string& base) {
    std::size_t slash = path.find_last_of("/\\");
    std::size_t name = slash == std::string::npos ? 0 : slash + 1;
    std::size_t dot = path.rfind('.');
    if (dot == std::string::npos || dot <= name) return K_UNKNOWN;  // no extension, or ".node"
    std::string ext = path.substr(dot + 1);
    for (char& c : ext) {
        if (c >= 'A' && c <= 'Z') c = static_cast<char>(c - 'A' + 'a');
    }
    base = path.substr(0, dot);
    if (ext == "node") return K_NODE;
    if (ext == "ele") return K_ELE;
    if (ext == "face") return K_FACE;
    if (ext == "neigh") return K_NEIGH;
    return K_UNKNOWN;
}

// Sends stdout to NUL for its lifetime: TetGen's loaders printf "Opening ..." and errors there.
class Quiet {
    int saved_;

public:
    Quiet() {
        std::fflush(stdout);
        saved_ = _dup(1);
        int nul = _open("NUL", _O_WRONLY);
        if (nul >= 0) {
            _dup2(nul, 1);
            _close(nul);
        }
    }
    ~Quiet() {
        std::fflush(stdout);
        if (saved_ >= 0) {
            _dup2(saved_, 1);
            _close(saved_);
        }
    }
    Quiet(const Quiet&) = delete;
    Quiet& operator=(const Quiet&) = delete;
};

// The file's number lines as tetgenio::readnumberline sees them (text mode, fgets in
// INPUTLINESIZE chunks, comment and blank lines skipped).
struct Scan {
    int lines = 0;
    std::string header_line;   // the whole buffer of the first number line (load_node's strstr)
    std::string header;        // the first number line from its first number-like character
    std::string second;        // the second number line, likewise
};

bool scan(tetgenio& io, const std::string& path, Scan& s) {
    FILE* f = std::fopen(path.c_str(), "r");
    if (!f) return false;
    std::vector<char> buf(INPUTLINESIZE + 1, '\0');
    char* p;
    while ((p = io.readnumberline(buf.data(), f, const_cast<char*>(path.c_str()))) != nullptr) {
        if (s.lines == 0) {
            s.header_line = buf.data();
            s.header = p;
        } else if (s.lines == 1) {
            s.second = p;
        }
        if (s.lines < INT_MAX) ++s.lines;
    }
    std::fclose(f);
    return true;
}

// Presets the context load_tet / load_face / our .neigh reader take from a .node.
void preset_context(tetgenio& io, const Scan& s) {
    if (s.lines >= 2) {
        long first = std::strtol(s.second.c_str(), nullptr, 0);
        if (first == 0 || first == 1) io.firstnumber = static_cast<int>(first);
    }
    io.numberofpoints = INT_MAX - 1;
}

std::string head(const char* kind, int first) {
    return std::string("tetgen ") + kind + "\nfirst " + std::to_string(first) + "\n";
}

int finish(const std::string& out) {
    std::fwrite(out.data(), 1, out.size(), stdout);
    return 0;
}

int dump_node(const std::string& path, std::string& base) {
    tetgenio io;
    Scan s;
    if (!scan(io, path, s) || s.lines < 1) return oracle_fail();
    long count;
    int available;
    if (std::strstr(s.header_line.c_str(), "rbox") != nullptr) {
        if (s.lines < 2) return oracle_fail();
        count = std::strtol(s.second.c_str(), nullptr, 0);
        available = s.lines - 2;
    } else {
        count = std::strtol(s.header_line.c_str(), nullptr, 0);  // load_node parses from the line start
        available = s.lines - 1;
    }
    if (count < 0 || count > available) return oracle_fail();
    bool ok;
    {
        Quiet q;
        try {
            ok = io.load_node(&base[0]);
        } catch (...) {
            ok = false;
        }
    }
    if (!ok) return oracle_fail();
    std::string out = head("node", io.firstnumber);
    out += "dim " + std::to_string(io.mesh_dim) + "\n";
    out += "attributes " + std::to_string(io.numberofpointattributes) + "\n";
    out += std::string("markers ") + (io.pointmarkerlist != nullptr ? "1" : "0") + "\n";
    out += "points " + std::to_string(io.numberofpoints) + "\n";
    for (int i = 0; i < io.numberofpoints; ++i) {
        out += f64_hex(io.pointlist[3 * i]) + " " + f64_hex(io.pointlist[3 * i + 1]) + " " +
               f64_hex(io.pointlist[3 * i + 2]);
        for (int j = 0; j < io.numberofpointattributes; ++j) {
            out += " " + f64_hex(io.pointattributelist[i * io.numberofpointattributes + j]);
        }
        if (io.pointmarkerlist != nullptr) out += " " + std::to_string(io.pointmarkerlist[i]);
        out += "\n";
    }
    return finish(out);
}

int dump_ele(const std::string& path, std::string& base) {
    tetgenio io;
    Scan s;
    if (!scan(io, path, s) || s.lines < 1) return oracle_fail();
    long count = std::strtol(s.header.c_str(), nullptr, 0);
    if (count > s.lines - 1) return oracle_fail();
    preset_context(io, s);
    bool ok;
    {
        Quiet q;
        try {
            ok = io.load_tet(&base[0]);
        } catch (...) {
            ok = false;
        }
    }
    if (!ok) return oracle_fail();
    std::string out = head("ele", io.firstnumber);
    out += "corners " + std::to_string(io.numberofcorners) + "\n";
    out += "attributes " + std::to_string(io.numberoftetrahedronattributes) + "\n";
    out += "tets " + std::to_string(io.numberoftetrahedra) + "\n";
    for (int i = 0; i < io.numberoftetrahedra; ++i) {
        for (int j = 0; j < io.numberofcorners; ++j) {
            if (j > 0) out += " ";
            out += std::to_string(io.tetrahedronlist[i * io.numberofcorners + j]);
        }
        for (int j = 0; j < io.numberoftetrahedronattributes; ++j) {
            out += " " + f64_hex(io.tetrahedronattributelist[i * io.numberoftetrahedronattributes + j]);
        }
        out += "\n";
    }
    return finish(out);
}

int dump_face(const std::string& path, std::string& base) {
    tetgenio io;
    Scan s;
    if (!scan(io, path, s) || s.lines < 1) return oracle_fail();
    long count = std::strtol(s.header.c_str(), nullptr, 0);
    if (count > s.lines - 1) return oracle_fail();
    preset_context(io, s);
    bool ok;
    {
        Quiet q;
        try {
            ok = io.load_face(&base[0]);
        } catch (...) {
            ok = false;
        }
    }
    if (!ok) return oracle_fail();
    std::string out = head("face", io.firstnumber);
    out += std::string("markers ") + (io.trifacemarkerlist != nullptr ? "1" : "0") + "\n";
    out += "faces " + std::to_string(io.numberoftrifaces) + "\n";
    for (int i = 0; i < io.numberoftrifaces; ++i) {
        out += std::to_string(io.trifacelist[3 * i]) + " " + std::to_string(io.trifacelist[3 * i + 1]) +
               " " + std::to_string(io.trifacelist[3 * i + 2]);
        if (io.trifacemarkerlist != nullptr) out += " " + std::to_string(io.trifacemarkerlist[i]);
        out += "\n";
    }
    return finish(out);
}

// TetGen has no .neigh loader. This follows load_tet (tetgen.cxx:445) line for line, with
// the corner check replaced by the neighbour rule outneighbors writes (tetgen.cxx:34978):
// -1 for a hull face, otherwise a tetrahedron number in [firstnumber, firstnumber + count).
int dump_neigh(const std::string& path) {
    tetgenio io;
    Scan s;
    if (!scan(io, path, s) || s.lines < 1) return oracle_fail();
    long declared = std::strtol(s.header.c_str(), nullptr, 0);
    if (declared > s.lines - 1) return oracle_fail();
    preset_context(io, s);
    FILE* f = std::fopen(path.c_str(), "r");
    if (!f) return oracle_fail();
    std::vector<char> buf(INPUTLINESIZE + 1, '\0');
    char* name = const_cast<char*>(path.c_str());
    char* p = io.readnumberline(buf.data(), f, name);
    int count = static_cast<int>(std::strtol(p, &p, 0));
    if (count <= 0) {
        std::fclose(f);
        return oracle_fail();
    }
    p = io.findnextnumber(p);
    int corners = (*p == '\0') ? 4 : static_cast<int>(std::strtol(p, &p, 0));
    if (corners != 4) {
        std::fclose(f);
        return oracle_fail();
    }
    std::vector<int> list(static_cast<std::size_t>(count) * 4);
    for (int i = 0; i < count; ++i) {
        p = io.readnumberline(buf.data(), f, name);
        for (int j = 0; j < 4; ++j) {
            p = io.findnextnumber(p);
            if (*p == '\0') {
                std::fclose(f);
                return oracle_fail();
            }
            int n = static_cast<int>(std::strtol(p, &p, 0));
            if (n != -1 && (n < io.firstnumber || n >= count + io.firstnumber)) {
                std::fclose(f);
                return oracle_fail();
            }
            list[static_cast<std::size_t>(i) * 4 + j] = n;
        }
    }
    std::fclose(f);
    std::string out = head("neigh", io.firstnumber);
    out += "tets " + std::to_string(count) + "\n";
    for (int i = 0; i < count; ++i) {
        const int* n = &list[static_cast<std::size_t>(i) * 4];
        out += std::to_string(n[0]) + " " + std::to_string(n[1]) + " " + std::to_string(n[2]) + " " +
               std::to_string(n[3]) + "\n";
    }
    return finish(out);
}

}  // namespace

int dump_tetgen(const char* path) {
    std::string p(path);
    std::string base;
    // FILENAMESIZE is 1024 and the loaders strcpy the base name plus an extension into it.
    if (p.size() >= FILENAMESIZE - 16) return oracle_fail();
    switch (kind_of(p, base)) {
        case K_NODE: return dump_node(p, base);
        case K_ELE: return dump_ele(p, base);
        case K_FACE: return dump_face(p, base);
        case K_NEIGH: return dump_neigh(p);
        default: return oracle_fail();
    }
}
