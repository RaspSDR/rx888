# SoapySDR Lib

if (MSVC)
    # Lib path
    set(POTHOS_DIR "C:/Program Files/PothosSDR")
    if (EXISTS "${POTHOS_DIR}")
        set(SoapySDR_FOUND TRUE)
        list(APPEND CMAKE_MODULE_PATH
            "${POTHOS_DIR}/cmake"
        )
        include(SoapySDRConfig)
        include(SoapySDRUtil)
    else ()
        set(SoapySDR_FOUND FALSE)
    endif ()
else (MSVC)
    find_package(SoapySDR CONFIG)
endif (MSVC)

if (SoapySDR_FOUND)
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

    target_link_libraries(sddc PRIVATE SDDC_CORE)
    set_target_properties(sddc PROPERTIES VERSION ${PROJECT_VERSION})
    set_target_properties(sddc PROPERTIES SOVERSION 1)

endif(SoapySDR_FOUND)