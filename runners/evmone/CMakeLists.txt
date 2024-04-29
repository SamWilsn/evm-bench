cmake_minimum_required(VERSION 3.18)

# Use CPM for CMake dependency management
include(cmake/CPM.cmake)

CPMAddPackage("gh:CLIUtils/CLI11@2.4.1")
CPMAddPackage("gh:ethereum/evmone@0.11.0")

project(evmone-runner LANGUAGES CXX C)

add_executable(runner runner.cpp)
target_link_libraries(runner PRIVATE CLI11::CLI11 evmone)
