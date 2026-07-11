# @dependency-start
# contract configuration
# responsibility Defines the optional C++ build configuration.
# upstream design cmake/README.md CMake layout guidance
# downstream design src/README.md C++ source layout guidance
# @dependency-end
cmake_minimum_required(VERSION 3.22)

project(project_template LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

if(CMAKE_INSTALL_PREFIX_INITIALIZED_TO_DEFAULT)
  set(
    CMAKE_INSTALL_PREFIX
    "${CMAKE_SOURCE_DIR}/.state/cpp-install/default"
    CACHE PATH "Default reusable install prefix for local C++ artifacts." FORCE
  )
endif()

set(
  PROJECT_TEMPLATE_CPP_BUILD_ROOT
  "${CMAKE_SOURCE_DIR}/build/cpp"
  CACHE PATH "Root for out-of-source C++ build trees."
)
set(
  PROJECT_TEMPLATE_CPP_INSTALL_ROOT
  "${CMAKE_SOURCE_DIR}/.state/cpp-install"
  CACHE PATH "Root for reusable local C++ install trees."
)

set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib")
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin")

include(CTest)

find_package(Python3 3.11 REQUIRED COMPONENTS Interpreter)

add_library(project_template_cpp INTERFACE)
target_include_directories(
  project_template_cpp
  INTERFACE
    "${CMAKE_CURRENT_SOURCE_DIR}/include"
)
target_compile_features(project_template_cpp INTERFACE cxx_std_17)
