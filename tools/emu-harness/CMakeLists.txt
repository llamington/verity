# SPDX-License-Identifier: GPL-3.0-only
#
# Headless melonDS harness. Links the melonDS emulation core (no Qt front end).
#
#   cmake -S tools/emu-harness -B tools/emu-harness/build -G Ninja -DCMAKE_BUILD_TYPE=Release
#   ninja -C tools/emu-harness/build
#
# MELONDS_DIR names a melonDS 1.1 source tree; without one (default ~/src/melonDS), CMake fetches
# tag 1.1 from GitHub.
cmake_minimum_required(VERSION 3.16)
project(verity_emu_harness CXX C)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(MELONDS_DIR "$ENV{HOME}/src/melonDS" CACHE PATH "melonDS source tree (pinned tag 1.1)")
if(NOT EXISTS "${MELONDS_DIR}/src/NDS.h")
    include(FetchContent)
    FetchContent_Declare(melonds
        GIT_REPOSITORY https://github.com/melonDS-emu/melonDS.git
        GIT_TAG 1.1
        GIT_SHALLOW TRUE)
    FetchContent_GetProperties(melonds)
    if(NOT melonds_POPULATED)
        FetchContent_Populate(melonds)
    endif()
    set(MELONDS_DIR "${melonds_SOURCE_DIR}")
endif()

# Core only: no Qt/SDL front end, no OpenGL, interpreter only (determinism), GDB stub kept.
set(BUILD_QT_SDL OFF CACHE BOOL "" FORCE)
set(ENABLE_OGLRENDERER OFF CACHE BOOL "" FORCE)
set(ENABLE_JIT OFF CACHE BOOL "" FORCE)
set(ENABLE_GDBSTUB ON CACHE BOOL "" FORCE)
set(ENABLE_LTO_RELEASE OFF CACHE BOOL "" FORCE)
add_subdirectory(${MELONDS_DIR} melonds EXCLUDE_FROM_ALL)

find_package(PNG REQUIRED)
find_package(Threads REQUIRED)

# LocalMP alone: net-utils would also pull in LAN, slirp and enet.
add_executable(emu-harness src/main.cpp src/platform.cpp src/json.cpp src/image.cpp src/elf.cpp
               ${MELONDS_DIR}/src/net/LocalMP.cpp)
target_link_libraries(emu-harness PRIVATE core PNG::PNG Threads::Threads)
# melonDS defines GDBSTUB_ENABLED with directory-scoped add_definitions, and it changes class
# layouts (NDS, ARM): the harness must see the same definition or objects are mis-sized.
target_compile_definitions(emu-harness PRIVATE GDBSTUB_ENABLED)
target_include_directories(emu-harness SYSTEM PRIVATE ${MELONDS_DIR}/src ${MELONDS_DIR}/src/net)
target_compile_options(emu-harness PRIVATE -Wall -Wextra)
set_source_files_properties(${MELONDS_DIR}/src/net/LocalMP.cpp PROPERTIES COMPILE_OPTIONS -w)
