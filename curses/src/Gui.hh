#pragma once

#include <vector>
#include <memory>
#include <functional>
#include <optional>
#include <tuple>
#include <string>
#include <snm_types.hh>
#include "Window.hh"

class Gui {
  public:
    using PropGetter = std::function<snm::ConnectionProps(const std::string&)>;
    using Connector = std::function<void(const snm::ConnectionId &)>;
    enum class Display {
        Networks,
        Props,
    };

    Gui();
    ~Gui();
    bool pressed(int ch);
    void setNetworkState(snm::ConnectionState &&state);
    void setNetworkStatus(snm::ConnectionStatus status);
    void setNetworkList(std::vector<snm::NetworkInfo> &&networks);
    void showProps(PropGetter getter);
    void showNetworks();
    void connect(Connector connector);

    std::tuple<std::string, snm::ConnectionProps> getProps();

    Display display() {
        return display_;
    }

  private:
    Display display_;

    std::vector<std::unique_ptr<Window>> windows_;

    unsigned cast(Display d) {
        return static_cast<unsigned>(d);
    }
};
