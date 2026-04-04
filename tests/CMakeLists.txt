cmake_minimum_required(VERSION 3.20)
project(ISimpa_Tests LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# GLM
include(FetchContent)
FetchContent_Declare(glm GIT_REPOSITORY https://github.com/g-truc/glm.git GIT_TAG 1.0.1)
FetchContent_MakeAvailable(glm)

add_executable(test_pipeline
    test_pipeline.cpp
    ../project/project.cpp
    ../project/solver.cpp
    ../project/result_parser.cpp
    ../mesh/scene_model.cpp
    ../mesh/ply_loader.cpp
    gpu_stubs.cpp
)

target_include_directories(test_pipeline PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/..
    ${CMAKE_CURRENT_SOURCE_DIR}/../glad/include
)
target_link_libraries(test_pipeline PRIVATE glm::glm)

if(WIN32)
    target_compile_definitions(test_pipeline PRIVATE NOMINMAX _CRT_SECURE_NO_WARNINGS)
endif()
