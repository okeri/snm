project("snm-package")
cmake_minimum_required(VERSION 3.1.0)
add_subdirectory("curses")
add_subdirectory("wait-online")

add_custom_target(snm ALL
  COMMAND CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR} cargo build --release
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/snm)

install(DIRECTORY share/etc/dbus-1 DESTINATION /etc)
install(DIRECTORY share/usr/lib DESTINATION ${CMAKE_INSTALL_PREFIX})
install(DIRECTORY share/usr/share DESTINATION ${CMAKE_INSTALL_PREFIX})
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/release/snm DESTINATION ${CMAKE_INSTALL_PREFIX}/bin)
