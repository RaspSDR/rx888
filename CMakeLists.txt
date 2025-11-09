cmake_minimum_required(VERSION 3.13)
project(SDDC VERSION 1.0.2 LANGUAGES C CXX)

include(CTest)

include(./cmake/CheckGit.cmake)
CheckGitSetup()

message(STATUS "CMAKE_SYSTEM_PROCESSOR=${CMAKE_SYSTEM_PROCESSOR}")

### build options
# default build type: Release
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release")
endif(NOT CMAKE_BUILD_TYPE)
message(STATUS "Build type: " ${CMAKE_BUILD_TYPE} " - Version: " ${VERSION} " / " ${LIBVER})

# allow disabling optimizations - for debug reasons
option(USE_SIMD_OPTIMIZATIONS "enable SIMD optimizations" ON)

# allow enabling address sanitizer - for debug reasons
if (CMAKE_CXX_COMPILER_ID STREQUAL "GNU")
    option(USE_DEBUG_ASAN "use GCC's address sanitizer?" OFF)
endif()

if (MSVC)
    set(CMAKE_CXX_FLAGS "-O2 /std:c++17 /EHsc /W3")

    get_filename_component(SDKPATH ${CMAKE_LINKER} DIRECTORY)
    find_program(LIBEXE lib HINTS ${SDKPATH} REQUIRED)

    # External Project FFTW on Windows
    if(${CMAKE_EXE_LINKER_FLAGS} MATCHES "X86")
        SET(FFTW_URL "ftp://ftp.fftw.org/pub/fftw/fftw-3.3.5-dll32.zip")
        SET(ARCH x86)
        SET(HASH 29882a43033c9393479a4df52a2e9120589c06a2b724155b1a682747fa3e57d4)
    else()
        SET(FFTW_URL "ftp://ftp.fftw.org/pub/fftw/fftw-3.3.5-dll64.zip")
        SET(ARCH x64)
        SET(HASH cfd88dc0e8d7001115ea79e069a2c695d52c8947f5b4f3b7ac54a192756f439f)
    endif()

    include(ExternalProject)
    ExternalProject_Add(
        LIBFFTW
        URL ${FFTW_URL}
        URL_HASH SHA256=${HASH}
        BUILD_IN_SOURCE TRUE
        DOWNLOAD_EXTRACT_TIMESTAMP TRUE
        CONFIGURE_COMMAND   ""
        BUILD_COMMAND       ${LIBEXE} /def:./libfftw3f-3.def /MACHINE:${ARCH} /OUT:./fftw3f-3.lib
        INSTALL_COMMAND     ""
    )
    ExternalProject_Get_Property(LIBFFTW SOURCE_DIR)
    SET(LIBFFTW_INCLUDE_DIRS ${SOURCE_DIR})
    SET(LIBFFTW_LIBRARY_DIRS ${SOURCE_DIR})
    SET(LIBFFTW_LIBRARIES fftw3f-3)
else()

    if (USE_DEBUG_ASAN)
        set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} -fsanitize=address")
        set(CMAKE_CXX_FLAGS "-O0 -std=c++17 -Wall")
        set(ASANLIB "asan")
    else()
        set(CMAKE_CXX_FLAGS "-O3 -std=c++17 -Wall")
    endif(USE_DEBUG_ASAN)
    #add_compile_options(-Wall -Wextra -pedantic)
    include(FindPkgConfig)
    pkg_check_modules(LIBUSB REQUIRED libusb-1.0)
    pkg_check_modules(LIBFFTW REQUIRED fftw3f)
endif (MSVC)

set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} -D_DEBUG")

add_subdirectory(src/core)
add_subdirectory(src/libsddc)
add_subdirectory(src/tests)

if (MSVC)
    add_subdirectory(src/extio)
endif()

find_package(SoapySDR CONFIG)

if (SoapySDR_FOUND)
    add_subdirectory(src/soapy)
endif(SoapySDR_FOUND)

# --- CPack packaging configuration ---------------------------------------
include(GNUInstallDirs)

set(CPACK_PACKAGE_NAME "sddc")
set(CPACK_PACKAGE_VENDOR "rx888")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "SDDC libraries and optional Soapy/ExtIO modules")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_CONTACT "support@rx-888.com")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/LICENSE.txt")

# Component-based packaging: produce separate packages for libsddc, soapy and extio
set(CPACK_COMPONENTS_ALL libsddc soapy extio)
set(CPACK_COMPONENT_INSTALL ON)

set(CPACK_COMPONENT_libsddc_DISPLAY_NAME "libsddc runtime")
set(CPACK_COMPONENT_libsddc_DESCRIPTION "libsddc shared library and headers")

set(CPACK_COMPONENT_soapy_DISPLAY_NAME "SoapySDR module")
set(CPACK_COMPONENT_soapy_DESCRIPTION "SoapySDR module for RX-888 family devices")

set(CPACK_COMPONENT_extio_DISPLAY_NAME "ExtIO (Windows)")
set(CPACK_COMPONENT_extio_DESCRIPTION "SDDC ExtIO GUI plugin for Windows")

# Platform-specific default generators
if(APPLE)
    set(CPACK_GENERATOR "TGZ;ZIP")
elseif(UNIX)
    # On Linux include RPM alongside tar.gz for distribution
    set(CPACK_GENERATOR "TGZ;ZIP;RPM")
else()
    # Windows: produce ZIP packages per-component
    set(CPACK_GENERATOR "ZIP")
endif()

# RPM-specific defaults that are useful on Linux
set(CPACK_RPM_PACKAGE_LICENSE "MIT")
set(CPACK_RPM_PACKAGE_RELEASE "1")

include(CPack)