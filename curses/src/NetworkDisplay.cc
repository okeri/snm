#include <algorithm>
#include <stdexcept>

#include "NetworkDisplay.hh"

namespace {

std::string formatStatus(snm::ConnectionStatus status) {
    switch (status) {
        case snm::ConnectionStatus::Initializing:
            return "Initializing";

        case snm::ConnectionStatus::Connecting:
            return "Connecting";

        case snm::ConnectionStatus::Authenticating:
            return "Authenticating";

        case snm::ConnectionStatus::GettingIP:
            return "Getting ip address";

        case snm::ConnectionStatus::AuthFail:
            return "Authentication failed";

        case snm::ConnectionStatus::Aborted:
            return "Aborted";

        case snm::ConnectionStatus::ConnectFail:
            return "Connection failed";

        default:
            return "Unknown status";
    }
}

}  // namespace

NetworkDisplay::NetworkDisplay() :
        Window(80, 24),
        state_(snm::State::NotConnected, "", false, 0, "") {
    int width, height;
    getmaxyx(win_, height, width);
    page_ = height - 3;
    update();
}

void NetworkDisplay::update() {
    int count = networks_.size();
    int current = -1;

    auto findCurrentNetwork = [this] () {
        auto found = std::find_if(networks_.begin(), networks_.end(),
                                  [this] (auto ni) {
                                      return state_.essid == ni.essid;
                                  });

        return found != networks_.end() ?
        std::distance(networks_.begin(), found) : -1;
    };

    if (state_.state != snm::State::NotConnected) {
        current = findCurrentNetwork();
    }

    if (!networks_.empty()) {
        selected_ = std::clamp(selected_, 0, count - 1);
    } else {
        selected_ = -1;
    }

    top_ = std::clamp(selected_ - page_ / 2, 0, std::max(count - page_, 0));

    werase(win_);
    auto utf_chars = [](const char *input) {
        unsigned result = 0;
        for (auto c = input; *c; ++c) {
            if ((*c & 0xc0) == 0x80) {
                result++;
            }
        }
        return result;
    };

    box(win_, 0, 0);
    if (count != 0) {
        auto head = colorControl(Colors::Head);
        mvwprintw(win_, 1, 1, "  %5s %48s %10s  Quality",
                  "Type", "Essid", "Secure");
        head.release();

        for (auto i = top_, max = std::min(count, top_ + page_); i < max; ++i) {
            auto clr = colorControl();
            if (selected_ == current && current == i) {
                clr = Colors::SelTagged;
            } else if (selected_ == i) {
                clr = Colors::Selected;
            } else if (current == i) {
                clr = Colors::Tagged;
            }

            auto essid = i != current ? networks_[i].essid :
                    currentConnectionInfo();
            mvwprintw(win_, i + 2 - top_, 1,
                      "%s%5s %*s %10s     %3d%%", selected_ == i ? "> ":"  ",
                      networks_[i].state == snm::State::Ethernet ?
                      "eth" : "wifi",
                      48 + utf_chars(essid.c_str()), essid.c_str(),
                      networks_[i].enc ? "secured" : "open",
                      networks_[i].quality);
        }
    } else {
        static const std::string outOfNetworks = "No netwoks found.";
        auto head = colorControl(Colors::Head);
        int width, height;
        getmaxyx(win_, height, width);

        mvwprintw(win_, height / 2, (width - outOfNetworks.length()) / 2,
                 outOfNetworks.c_str());
    }

    update_panels();
    doupdate();
}

void NetworkDisplay::assign(Networks &&networks) {
    networks_ = networks;
    update();
}

void NetworkDisplay::setState(snm::ConnectionState &&state) {
    state_ = state;
    update();
}

void NetworkDisplay::setStatus(snm::ConnectionStatus status) {
    status_ = status;
    update();
}

std::string NetworkDisplay::currentConnectionInfo() {
    switch (state_.state) {
        case snm::State::Ethernet:
        case snm::State::Wifi:
            return state_.essid + " [" + state_.ip + "]";

        case snm::State::ConnectingEth:
        case snm::State::ConnectingWifi:
            return state_.essid + " (" + formatStatus(status_) + ")";

        default:
            return state_.essid;
    }
}

std::optional<snm::NetworkInfo> NetworkDisplay::selectedNetwork() {
    return selected_ != -1 ?
            std::optional(networks_[selected_]) :
            std::nullopt;
}

bool NetworkDisplay::pressed(int ch) {
    switch (ch) {
        case KEY_DOWN:
            selected_++;
            update();
            return true;

        case KEY_UP:
            selected_--;
            update();
            return true;

        case KEY_NPAGE:
            selected_ += page_;
            update();
            return true;

        case KEY_PPAGE:
            selected_ -= page_;
            update();
            return true;

        default:
            return false;
    }
}
