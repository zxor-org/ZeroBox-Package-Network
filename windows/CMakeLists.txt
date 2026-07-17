cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "zerobox_network")
project(${PROJECT_NAME} LANGUAGES C CXX)

include("${CMAKE_CURRENT_SOURCE_DIR}/../cmake/ZeroBoxNetworkPrebuilt.cmake")

if(NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
  message(FATAL_ERROR "ZeroBox Network supports 64-bit Windows builds only")
endif()

set(PREBUILT_DIR "${CMAKE_CURRENT_BINARY_DIR}/prebuilt")
zerobox_network_fetch("zerobox-network-windows-x86_64.zip" "${PREBUILT_DIR}")

add_library(zerobox_network SHARED IMPORTED GLOBAL)
set_target_properties(zerobox_network PROPERTIES
  IMPORTED_IMPLIB "${PREBUILT_DIR}/lib/zerobox_network.dll.lib"
  IMPORTED_LOCATION "${PREBUILT_DIR}/lib/zerobox_network.dll")

set(zerobox_network_bundled_libraries
  "${PREBUILT_DIR}/lib/zerobox_network.dll"
  PARENT_SCOPE)
