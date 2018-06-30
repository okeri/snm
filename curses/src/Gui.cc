#include "Gui.hh"
#include "NetworkDisplay.hh"
#include "NetworkProps.hh"


Gui::Gui() {
    initscr();
    start_color();
    cbreak();
    noecho();
    curs_set(0);
    keypad(stdscr, TRUE);
    init_pair(Colors::Selected, COLOR_BLACK, COLOR_WHITE);
    init_pair(Colors::Tagged, COLOR_GREEN, COLOR_BLACK);
    init_pair(Colors::SelTagged, COLOR_BLACK, COLOR_GREEN);
    init_pair(Colors::Head, COLOR_RED, COLOR_BLACK);
    windows_.emplace_back(std::make_unique<NetworkDisplay>());
    windows_.emplace_back(std::make_unique<NetworkProps>());
}

bool Gui::pressed(int ch) {
    return windows_[static_cast<unsigned>(display_)]->pressed(ch);
}

void Gui::setNetworkState(snm::ConnectionState &&state) {
    auto networkDisplay = static_cast<NetworkDisplay*>(
        windows_[cast(Display::Networks)].get());
    networkDisplay->setState(std::move(state));
}

void Gui::setNetworkStatus(snm::ConnectionStatus status) {
    auto networkDisplay = static_cast<NetworkDisplay*>(
        windows_[cast(Display::Networks)].get());
    networkDisplay->setStatus(status);
}

void Gui::setNetworkList(std::vector<snm::NetworkInfo> &&networks) {
    auto networkDisplay = static_cast<NetworkDisplay*>(
        windows_[cast(Display::Networks)].get());
    networkDisplay->assign(std::move(networks));
}

std::tuple<std::string, snm::ConnectionProps> Gui::getProps() {
    auto networkProps  = static_cast<NetworkProps*>(
        windows_[cast(Display::Props)].get());
    return networkProps->get();
}

void Gui::showProps(PropGetter getter) {
    auto networkDisplay = static_cast<NetworkDisplay*>(
        windows_[cast(Display::Networks)].get());
    auto networkProps  = static_cast<NetworkProps*>(
        windows_[cast(Display::Props)].get());

    if (auto net = networkDisplay->selectedNetwork();
        net && net.value().state == snm::State::Wifi) {
        networkProps->assign(net.value().essid, getter(net.value().essid));
        display_ = Display::Props;
        networkProps->setTop();
    }
}

void Gui::connect(Connector connector) {
    auto networkDisplay = static_cast<NetworkDisplay*>(
        windows_[cast(Display::Networks)].get());
    if (auto net = networkDisplay->selectedNetwork(); net) {
        connector(snm::ConnectionId(net.value()));
    }
}

void Gui::showNetworks() {
    display_ = Display::Networks;
    windows_[cast(display_)]->setTop();
}

Gui::~Gui() {
    endwin();
}
