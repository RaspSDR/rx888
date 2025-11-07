cmake_minimum_required(VERSION 3.13)

file(GLOB UNITTESTS "./*.cpp")

add_executable(unittest ${UNITTESTS})

target_include_directories(unittest PUBLIC "${LIBFFTW_INCLUDE_DIR}")
target_link_directories(unittest PUBLIC "${LIBFFTW_LIBRARY_DIRS}")

include_directories("." "../core")

target_link_libraries(unittest PRIVATE SDDC_CORE)
if (MSVC)
else()
  target_link_libraries(unittest PUBLIC pthread ${ASANLIB})
endif (MSVC)


foreach(TESTSRC ${UNITTESTS})
    file(STRINGS ${TESTSRC} TESTS REGEX "^TEST_CASE\(.+\)")
    foreach(TEST ${TESTS})
        if (${TEST} MATCHES "TEST_CASE\\(([^,]+)[\\s,]*([^\\)]+)\\)")
            set(FIXTURE ${CMAKE_MATCH_1})
            set(TESTNAME ${CMAKE_MATCH_2})
            string(STRIP ${FIXTURE} FIXTURE)
            string(STRIP ${TESTNAME} TESTNAME)
            add_test(${FIXTURE}_${TESTNAME} unittest ${FIXTURE}::${TESTNAME})
        endif()
    endforeach()
endforeach()
