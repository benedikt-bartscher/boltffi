cmake_minimum_required(VERSION 3.20)
project(boltffi_c_demo LANGUAGES C)

find_path(DEMO_INCLUDE_DIR demo.h
    PATHS "${CMAKE_CURRENT_SOURCE_DIR}/generated/include"
    NO_DEFAULT_PATH REQUIRED)
find_file(DEMO_SHARED_LIBRARY
    NAMES "${CMAKE_SHARED_LIBRARY_PREFIX}demo${CMAKE_SHARED_LIBRARY_SUFFIX}"
    PATHS "${CMAKE_CURRENT_SOURCE_DIR}/generated/lib"
    NO_DEFAULT_PATH REQUIRED)

add_library(demo SHARED IMPORTED)
set_target_properties(demo PROPERTIES
    IMPORTED_LOCATION "${DEMO_SHARED_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${DEMO_INCLUDE_DIR}")

if(WIN32)
    find_file(DEMO_IMPORT_LIBRARY
        NAMES demo.dll.lib libdemo.dll.a
        PATHS "${CMAKE_CURRENT_SOURCE_DIR}/generated/lib"
        NO_DEFAULT_PATH REQUIRED)
    set_target_properties(demo PROPERTIES IMPORTED_IMPLIB "${DEMO_IMPORT_LIBRARY}")
endif()

add_executable(c_demo
    tests/demo.c
    tests/scalars.c
    tests/results.c
    tests/classes.c
    tests/options.c
    tests/strings.c
    tests/records.c
    tests/vectors.c
    tests/builtins.c
    tests/enums.c
    tests/owned_records.c
    tests/constants.c
    tests/callbacks.c
    tests/scalar_enums.c
)
set_target_properties(c_demo PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED YES C_EXTENSIONS NO)
target_link_libraries(c_demo PRIVATE demo)

if(WIN32)
    add_custom_command(TARGET c_demo POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different
        "$<TARGET_FILE:demo>" "$<TARGET_FILE_DIR:c_demo>")
else()
    target_link_libraries(c_demo PRIVATE m)
endif()

enable_testing()
add_test(NAME c_demo COMMAND c_demo)
