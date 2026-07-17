set(ZEROBOX_NETWORK_VERSION "0.1.0")
set(ZEROBOX_NETWORK_RELEASE_BASE
    "https://github.com/zxor-org/ZeroBox-Package-Network/releases/download/v${ZEROBOX_NETWORK_VERSION}")

function(zerobox_network_fetch ASSET OUTPUT_DIR)
  file(MAKE_DIRECTORY "${OUTPUT_DIR}")
  set(ARCHIVE "${OUTPUT_DIR}/${ASSET}")
  set(CHECKSUM_FILE "${ARCHIVE}.sha256")

  if(NOT EXISTS "${ARCHIVE}")
    file(DOWNLOAD
      "${ZEROBOX_NETWORK_RELEASE_BASE}/${ASSET}"
      "${ARCHIVE}"
      STATUS DOWNLOAD_STATUS
      TLS_VERIFY ON
      SHOW_PROGRESS)
    list(GET DOWNLOAD_STATUS 0 DOWNLOAD_CODE)
    if(NOT DOWNLOAD_CODE EQUAL 0)
      list(GET DOWNLOAD_STATUS 1 DOWNLOAD_MESSAGE)
      file(REMOVE "${ARCHIVE}")
      message(FATAL_ERROR "Unable to download ${ASSET}: ${DOWNLOAD_MESSAGE}")
    endif()
  endif()

  file(DOWNLOAD
    "${ZEROBOX_NETWORK_RELEASE_BASE}/${ASSET}.sha256"
    "${CHECKSUM_FILE}"
    STATUS CHECKSUM_STATUS
    TLS_VERIFY ON)
  list(GET CHECKSUM_STATUS 0 CHECKSUM_CODE)
  if(NOT CHECKSUM_CODE EQUAL 0)
    message(FATAL_ERROR "Unable to download checksum for ${ASSET}")
  endif()

  file(READ "${CHECKSUM_FILE}" EXPECTED_HASH)
  string(REGEX MATCH "^[0-9a-fA-F]+" EXPECTED_HASH "${EXPECTED_HASH}")
  file(SHA256 "${ARCHIVE}" ACTUAL_HASH)
  if(NOT ACTUAL_HASH STREQUAL EXPECTED_HASH)
    file(REMOVE "${ARCHIVE}")
    message(FATAL_ERROR "Checksum mismatch for ${ASSET}")
  endif()

  set(EXTRACT_MARKER "${OUTPUT_DIR}/.${ASSET}.extracted")
  if(NOT EXISTS "${EXTRACT_MARKER}")
    file(REMOVE_RECURSE "${OUTPUT_DIR}/lib" "${OUTPUT_DIR}/include")
    execute_process(
      COMMAND "${CMAKE_COMMAND}" -E tar xf "${ARCHIVE}"
      WORKING_DIRECTORY "${OUTPUT_DIR}"
      RESULT_VARIABLE EXTRACT_RESULT)
    if(NOT EXTRACT_RESULT EQUAL 0)
      message(FATAL_ERROR "Unable to extract ${ASSET}")
    endif()
    file(WRITE "${EXTRACT_MARKER}" "${ACTUAL_HASH}\n")
  endif()
endfunction()
