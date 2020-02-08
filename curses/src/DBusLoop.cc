#include "DBusLoop.hh"

DBusLoop::DBusLoop() : dispatcherThread_([] { dispatcher_.enter(); }) {
}

DBusLoop::~DBusLoop() {
    dispatcher_.leave();
    dispatcherThread_.join();
}

DBus::Connection DBusLoop::connection(Bus bus) {
    DBus::default_dispatcher = &dispatcher_;
    switch (bus) {
        case Bus::System:
            return DBus::Connection::SystemBus();
        case Bus::Session:
            return DBus::Connection::SessionBus();
    }
    throw std::logic_error("Unknown bus");
}

DBus::BusDispatcher DBusLoop::dispatcher_;
