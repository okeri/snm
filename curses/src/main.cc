#include <snm.hh>
#include <locale.h>
#include "DBusLoop.hh"
#include "Gui.hh"

int main(int argc, char *argv[]) {
    setlocale(LC_ALL, "");

    Gui gui;
    snm::NetworkManager networkManager(
        DBusLoop::connection(DBusLoop::Bus::System),
        [&gui] (snm::ConnectionState&& state) {
            gui.setNetworkState(std::move(state));
        },
        [&gui] (snm::ConnectionStatus status) {
            gui.setNetworkStatus(status);
        },
        [&gui] (std::vector<snm::NetworkInfo>&& networks) {
            gui.setNetworkList(std::move(networks));
        });

    gui.setNetworkState(networkManager.get_state());
    gui.setNetworkList(networkManager.get_networks());
    gui.showNetworks();

    auto connect = [&networkManager, &gui] () {
        gui.connect([&networkManager] (const snm::ConnectionId & id) {
                networkManager.connect(id);
            });
    };

    auto storeProps = [&networkManager, &gui] () {
        auto [essid, props] = gui.getProps();
        networkManager.set_props(essid, props);
        gui.showNetworks();
    };

    DBusLoop loop;
    while (true) {
        auto ch = getch();
        if (auto handled = gui.pressed(ch); !handled) {
            switch (ch) {
                case KEY_LEFT:
                    if (gui.display() == Gui::Display::Props) {
                        gui.showNetworks();
                    }
                    break;

                case 'P': case 'p':
                case KEY_RIGHT:
                    if (gui.display() == Gui::Display::Networks) {
                        gui.showProps(
                            [&networkManager] (const std::string &essid) {
                                return networkManager.get_props(essid);
                            });
                    } else {
                        storeProps();
                    }
                    break;

                case KEY_ESC:
                    if (gui.display() == Gui::Display::Props) {
                        gui.showNetworks();
                    } else {
                        return 0;
                    }

                case 'C': case 'c':
                    connect();
                    break;

                case 'D': case 'd':
                    networkManager.disconnect();
                    break;

                case KEY_APPLY:
                    if (gui.display() == Gui::Display::Networks) {
                        connect();
                    } else {
                        storeProps();
                    }
                    break;

                case 'Q': case 'q':
                    return 0;
            }
        }
    }
    return 0;
}
