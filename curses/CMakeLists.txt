project("cde")
cmake_minimum_required(VERSION 3.1.0)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_STANDARD 17)

find_package(PkgConfig)
pkg_check_modules(DBUS dbus-c++-1)
include_directories(${DBUS_INCLUDE_DIRS} include)
link_directories(${DBUS_LIBRARY_DIRS})

add_executable(snm-curses
  src/main.cc
  src/Gui.cc
  src/Window.cc
  src/NetworkDisplay.cc
  src/NetworkProps.cc
  src/DBusLoop.cc)

target_link_libraries(snm-curses ${DBUS_LIBRARIES} -lncursesw -lformw -lpanelw -lpthread)
SET(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wall -pedantic")
