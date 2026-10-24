cmake_minimum_required(VERSION 3.25)
project(sega_llvm_orc_jit LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(LLVM REQUIRED CONFIG)

message(STATUS "Using LLVM ${LLVM_PACKAGE_VERSION}")
message(STATUS "LLVM CMake directory: ${LLVM_DIR}")

add_executable(llvm_orc_smoke src/orc_smoke.cpp)
target_include_directories(llvm_orc_smoke SYSTEM PRIVATE ${LLVM_INCLUDE_DIRS})
add_definitions(${LLVM_DEFINITIONS})

llvm_map_components_to_libnames(LLVM_LIBS
    Core
    OrcJIT
    ExecutionEngine
    RuntimeDyld
    native
    nativeasmparser
    nativecodegen
    Support
)
target_link_libraries(llvm_orc_smoke PRIVATE ${LLVM_LIBS})

enable_testing()
add_test(NAME llvm_orc_smoke COMMAND llvm_orc_smoke)
