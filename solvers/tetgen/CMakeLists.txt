# Builds tetgen.exe from the vendored WIAS TetGen 1.5.0 (third_party/tetgen-1.5.0) with the
# settings upstream's own src/tetgen target has at 929a5c8:
#   - add_executable(tetgen tetgen.cxx predicates.cxx): the command-line program, predicates as
#     a second translation unit, no TETLIBRARY (src/tetgen/CMakeLists.txt at 929a5c8);
#   - no flags of its own: upstream's top-level CMakeLists adds none for MSVC, so the target gets
#     CMake's defaults under cmake_minimum_required(VERSION 3.10) (CMakeLists.txt:3), which fixes
#     the policies that decide the MSVC defaults (/W3 and /MD stay in the default flags).
# solvers/build.ps1 configures upstream's tree in the same run and refuses the build unless the
# cl and link command lines MSBuild records for this target equal those of upstream's tetgen
# target, paths aside.
cmake_minimum_required(VERSION 3.10 FATAL_ERROR)
cmake_policy(SET CMP0177 NEW)
cmake_policy(SET CMP0175 NEW)
project(tetgen150)

set(TETGEN_SOURCE_DIR "${CMAKE_CURRENT_LIST_DIR}/../../third_party/tetgen-1.5.0"
    CACHE PATH "the vendored WIAS TetGen 1.5.0 sources")

set_property(GLOBAL PROPERTY USE_FOLDERS ON)
add_executable(tetgen "${TETGEN_SOURCE_DIR}/tetgen.cxx" "${TETGEN_SOURCE_DIR}/predicates.cxx")
set_property(TARGET tetgen PROPERTY FOLDER "executables")
