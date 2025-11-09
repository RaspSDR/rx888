cmake_minimum_required(VERSION 3.13)

if (MSVC)
  set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS TRUE)
endif (MSVC)

include_directories("." "../core")

add_library(sddc SHARED
    libsddc.cpp
)

target_link_libraries(sddc PRIVATE SDDC_CORE)
set_target_properties(sddc PROPERTIES VERSION ${PROJECT_VERSION})
set_target_properties(sddc PROPERTIES SOVERSION 0)

add_library(wavewriter STATIC wavewrite.c)

include(GNUInstallDirs)

# Install shared library and public headers so libsddc can be packaged
install(TARGETS sddc
  LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
  RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
  ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
  COMPONENT libsddc
)

# Install public headers from this directory (headers only)
install(DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/
  DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/sddc
  COMPONENT libsddc
  FILES_MATCHING PATTERN "*.h"
)

# applications
add_executable(sddc_test sddc_test.c)
target_link_libraries(sddc_test PRIVATE sddc ${ASANLIB})

add_executable(sddc_stream_test sddc_stream_test.c)
target_link_libraries(sddc_stream_test sddc ${ASANLIB} wavewriter)

add_executable(sddc_vhf_stream_test sddc_vhf_stream_test.c)
target_link_libraries(sddc_vhf_stream_test sddc ${ASANLIB} wavewriter)
