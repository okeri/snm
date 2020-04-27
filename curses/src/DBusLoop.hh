#pragma once

#include <thread>

#include <dbus-c++/dbus.h>

class DBusLoop {
    std::thread dispatcherThread_;

    static DBus::BusDispatcher dispatcher_;

  public:
    enum class Bus { System, Session };

    DBusLoop();
    ~DBusLoop();

    static DBus::Connection connection(Bus bus);
};
