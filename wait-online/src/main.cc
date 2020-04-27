#include <dbus-c++/dbus.h>
#include <snm.hh>

DBus::BusDispatcher dispatcher;

int main(int argc, char* argv[]) {
    DBus::default_dispatcher = &dispatcher;

    auto online = [](snm::ConnectionState&& state) {
        return state.state == snm::State::Ethernet ||
               state.state == snm::State::Wifi;
    };

    snm::NetworkManager networkManager(
        DBus::Connection::SystemBus(),
        [&](snm::ConnectionState&& state) {
            if (online(std::move(state))) {
                dispatcher.leave();
            }
        },
        [](snm::ConnectionStatus status) {},
        [](std::vector<snm::NetworkInfo>&& networks) {});
    try {
        if (online(networkManager.get_state())) {
            return 0;
        }
    } catch (DBus::Error& error) {
        std::cerr << error.what() << std::endl;
        return -1;
    };
    dispatcher.enter();
    return 0;
}
