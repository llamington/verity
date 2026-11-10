# SPDX-License-Identifier: GPL-3.0-only
#
# Headless melonDS harness (PLAN §7). Links the melonDS emulation core (no Qt front end).
#
#   cmake -S tools/emu-harness -B tools/emu-harness/build -G Ninja \
#         -DMELONDS_DIR=$HOME/src/melonDS -DCMAKE_BUILD_TYPE=Release
#   ninja -C tools/emu-harness/build
cmake_minimum_required(VERSION 3.16)
project(verity_emu_harness CXX C)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(MELONDS_DIR "$ENV{HOME}/src/melonDS" CACHE PATH "melonDS source tree (pinned tag 1.1)")

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
add_executable(emu-harness src/main.cpp src/platform.cpp src/json.cpp ${MELONDS_DIR}/src/net/LocalMP.cpp)
target_link_libraries(emu-harness PRIVATE core PNG::PNG Threads::Threads)
# melonDS defines GDBSTUB_ENABLED with directory-scoped add_definitions, and it changes class
# layouts (NDS, ARM): the harness must see the same definition or objects are mis-sized.
target_compile_definitions(emu-harness PRIVATE GDBSTUB_ENABLED)
target_include_directories(emu-harness SYSTEM PRIVATE ${MELONDS_DIR}/src ${MELONDS_DIR}/src/net)
target_compile_options(emu-harness PRIVATE -Wall -Wextra)
set_source_files_properties(${MELONDS_DIR}/src/net/LocalMP.cpp PROPERTIES COMPILE_OPTIONS -w)
