project("snm-wait-online")
cmake_minimum_required(VERSION 3.1.0)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wall -pedantic")

find_package(PkgConfig)
pkg_check_modules(DBUS dbus-c++-1)
include_directories(${DBUS_INCLUDE_DIRS} ../include/snm)
link_directories(${DBUS_LIBRARY_DIRS})

add_executable(snm-wait-online
  src/main.cc)

target_link_libraries(snm-wait-online ${DBUS_LIBRARIES} -lpthread)
install(TARGETS snm-wait-online RUNTIME)
