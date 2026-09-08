cmake_minimum_required(VERSION 3.20)
project(ISimpa_NewGUI VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ─── Fetch Dependencies ──────────────────────────────────────────────────────

include(FetchContent)

# GLFW
FetchContent_Declare(
    glfw
    GIT_REPOSITORY https://github.com/glfw/glfw.git
    GIT_TAG        3.4
)
set(GLFW_BUILD_DOCS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(GLFW_INSTALL OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(glfw)

# Dear ImGui (docking branch, PINNED to a commit)
# Tracking the branch made this repo unbuildable between April and September
# 2026 without a single commit landing here: upstream retired BeginChildFrame()
# and CaptureMouseFromApp(), which ImGuizmo still called. Move this SHA
# deliberately, never implicitly.
FetchContent_Declare(
    imgui
    GIT_REPOSITORY https://github.com/ocornut/imgui.git
    GIT_TAG        a2b7d6e7928e92a0b764d7bd8f3faceb660e60a7  # docking @ 2026-09-07
)
FetchContent_MakeAvailable(imgui)

# ImPlot (charts, PINNED)
FetchContent_Declare(
    implot
    GIT_REPOSITORY https://github.com/epezent/implot.git
    GIT_TAG        7eeb9168d2e5e6b14e266d8782ecf7e649dfc3a4  # master @ 2026-06-08
)
FetchContent_MakeAvailable(implot)

# ImGuizmo: REMOVED 2026-09-08. It was fetched, compiled and linked while being
# called nowhere in this codebase -- its only trace was a word in the About box.
# It was also the sole reason a clean checkout could no longer build.

# GLM (math)
FetchContent_Declare(
    glm
    GIT_REPOSITORY https://github.com/g-truc/glm.git
    GIT_TAG        1.0.1
)
FetchContent_MakeAvailable(glm)

# ─── glad (OpenGL 4.6 loader) ────────────────────────────────────────────────
# We bundle a minimal glad loader generated for GL 4.6 core
add_library(glad STATIC
    ${CMAKE_CURRENT_SOURCE_DIR}/glad/src/glad.c
)
target_include_directories(glad PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/glad/include)

# ─── ImGui library ───────────────────────────────────────────────────────────

add_library(imgui_lib STATIC
    ${imgui_SOURCE_DIR}/imgui.cpp
    ${imgui_SOURCE_DIR}/imgui_demo.cpp
    ${imgui_SOURCE_DIR}/imgui_draw.cpp
    ${imgui_SOURCE_DIR}/imgui_tables.cpp
    ${imgui_SOURCE_DIR}/imgui_widgets.cpp
    ${imgui_SOURCE_DIR}/backends/imgui_impl_glfw.cpp
    ${imgui_SOURCE_DIR}/backends/imgui_impl_opengl3.cpp
)
target_include_directories(imgui_lib PUBLIC
    ${imgui_SOURCE_DIR}
    ${imgui_SOURCE_DIR}/backends
)
target_link_libraries(imgui_lib PUBLIC glfw glad)
target_compile_definitions(imgui_lib PUBLIC IMGUI_IMPL_OPENGL_LOADER_GLAD)

# ─── ImPlot library ──────────────────────────────────────────────────────────

add_library(implot_lib STATIC
    ${implot_SOURCE_DIR}/implot.cpp
    ${implot_SOURCE_DIR}/implot_items.cpp
)
target_include_directories(implot_lib PUBLIC ${implot_SOURCE_DIR})
target_link_libraries(implot_lib PUBLIC imgui_lib)

# ─── Main Application ────────────────────────────────────────────────────────

set(SOURCES
    main.cpp
    mesh/miniz.c
    mesh/miniz_tdef.c
    mesh/miniz_tinfl.c
    mesh/miniz_zip.c
    app/app.cpp
    app/theme.cpp
    app/workflow_rail.cpp
    panels/outliner.cpp
    panels/properties.cpp
    panels/console.cpp
    panels/results.cpp
    panels/materials.cpp
    viewport/viewport.cpp
    viewport/camera.cpp
    commands/command_palette.cpp
    mesh/scene_model.cpp
    mesh/ply_loader.cpp
    mesh/gpu_mesh.cpp
    app/selection.cpp
    project/project.cpp
    project/solver.cpp
    project/result_parser.cpp
)

add_executable(ISimpa_NewGUI WIN32 ${SOURCES})

target_include_directories(ISimpa_NewGUI PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(ISimpa_NewGUI PRIVATE
    imgui_lib
    implot_lib
    glm::glm
    glad
    glfw
    opengl32
    dwmapi
    shell32
    gdiplus
)

# Copy solver executables to output directory
# Search multiple locations: build_solvers output, currentRelease, or installed solvers
set(SOLVER_SEARCH_PATHS
    "${CMAKE_CURRENT_SOURCE_DIR}/../../build_solvers"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../currentRelease"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src/isimpa"
)

set(SOLVER_NAMES spps classicalTheory preprocess tetgen)
set(SOLVER_OUTPUT_DIR "$<TARGET_FILE_DIR:ISimpa_NewGUI>/solvers")

add_custom_command(TARGET ISimpa_NewGUI POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E make_directory ${SOLVER_OUTPUT_DIR}
    COMMENT "Ensuring solvers directory exists..."
)

foreach(SOLVER_PATH ${SOLVER_SEARCH_PATHS})
    if(EXISTS ${SOLVER_PATH})
        # Copy any solver executables found in Release subdirectories
        file(GLOB_RECURSE FOUND_SOLVERS "${SOLVER_PATH}/**/Release/*.exe")
        foreach(SOLVER_EXE ${FOUND_SOLVERS})
            get_filename_component(SOLVER_NAME ${SOLVER_EXE} NAME)
            add_custom_command(TARGET ISimpa_NewGUI POST_BUILD
                COMMAND ${CMAKE_COMMAND} -E copy_if_different
                    "${SOLVER_EXE}"
                    ${SOLVER_OUTPUT_DIR}/${SOLVER_NAME}
                COMMENT "Copying ${SOLVER_NAME}..."
            )
        endforeach()
        # Also try direct copy if it's a flat directory
        file(GLOB FLAT_SOLVERS "${SOLVER_PATH}/*.exe")
        foreach(SOLVER_EXE ${FLAT_SOLVERS})
            get_filename_component(SOLVER_NAME ${SOLVER_EXE} NAME)
            add_custom_command(TARGET ISimpa_NewGUI POST_BUILD
                COMMAND ${CMAKE_COMMAND} -E copy_if_different
                    "${SOLVER_EXE}"
                    ${SOLVER_OUTPUT_DIR}/${SOLVER_NAME}
                COMMENT "Copying ${SOLVER_NAME}..."
            )
        endforeach()
    endif()
endforeach()

# Also copy logo assets next to the executable
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/isimpa_neon_logo.png")
    add_custom_command(TARGET ISimpa_NewGUI POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            "${CMAKE_CURRENT_SOURCE_DIR}/isimpa_neon_logo.png"
            "$<TARGET_FILE_DIR:ISimpa_NewGUI>/isimpa_neon_logo.png"
        COMMENT "Copying logo..."
    )
endif()
