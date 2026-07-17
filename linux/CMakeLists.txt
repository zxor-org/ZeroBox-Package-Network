cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "zerobox_network")
project(${PROJECT_NAME} LANGUAGES C CXX)

include("${CMAKE_CURRENT_SOURCE_DIR}/../cmake/ZeroBoxNetworkPrebuilt.cmake")

if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64)$")
  set(ZEROBOX_NETWORK_ARCH "aarch64")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
  set(ZEROBOX_NETWORK_ARCH "x86_64")
else()
  message(FATAL_ERROR "Unsupported Linux architecture: ${CMAKE_SYSTEM_PROCESSOR}")
endif()

set(PREBUILT_DIR "${CMAKE_CURRENT_BINARY_DIR}/prebuilt")
zerobox_network_fetch(
  "zerobox-network-linux-${ZEROBOX_NETWORK_ARCH}.tar.gz"
  "${PREBUILT_DIR}")

add_library(zerobox_network SHARED IMPORTED GLOBAL)
set_target_properties(zerobox_network PROPERTIES
  IMPORTED_LOCATION "${PREBUILT_DIR}/lib/libzerobox_network.so")

set(zerobox_network_bundled_libraries
  "${PREBUILT_DIR}/lib/libzerobox_network.so"
  PARENT_SCOPE)
