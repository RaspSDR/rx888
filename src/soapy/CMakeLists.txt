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
    include_directories("." "../libsddc")

    SOAPY_SDR_MODULE_UTIL(
        TARGET SDDCSupport
        SOURCES
        SoapySDDC.hpp
        Registration.cpp
        Settings.cpp
        Streaming.cpp
    )

    add_dependencies(SDDCSupport sddc)

    target_link_libraries(SDDCSupport PUBLIC sddc)
    set_target_properties(SDDCSupport PROPERTIES VERSION ${PROJECT_VERSION})
    set_target_properties(SDDCSupport PROPERTIES SOVERSION 1)

endif(SoapySDR_FOUND)
