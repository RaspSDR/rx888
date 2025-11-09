# SoapySDR Lib
include_directories("." "../core")

SOAPY_SDR_MODULE_UTIL(
    TARGET SDDCSupport
    SOURCES
    SoapySDDC.hpp
    Registration.cpp
    Settings.cpp
    Streaming.cpp
    LIBRARIES
    SDDC_CORE
)

add_dependencies(SDDCSupport SDDC_CORE)

# Link the generated module target to the core library. The macro above
# creates a target named SDDCSupport (case-sensitive), so ensure we use
# that same name here.
target_link_libraries(SDDCSupport PRIVATE SDDC_CORE)
if(NOT APPLE)
    set_target_properties(SDDCSupport PROPERTIES VERSION ${PROJECT_VERSION})
    set_target_properties(SDDCSupport PROPERTIES SOVERSION 1)
endif()

# Install rules so the built Soapy module and its public header can be
# packaged and installed. Use GNUInstallDirs for portable install paths.
include(GNUInstallDirs)

install(TARGETS SDDCSupport
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}/SoapySDR/modules-1
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
    COMPONENT soapy
)
