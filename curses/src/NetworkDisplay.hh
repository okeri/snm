#pragma once

#include <memory>
#include <vector>
#include <string>
#include <snm_types.hh>

#include "Window.hh"

class NetworkDisplay : public Window {
    using Networks = std::vector<snm::NetworkInfo>;

    int selected_ = 0;
    int top_ = 0;
    int page_;
    Networks networks_;
    snm::ConnectionState state_;
    snm::ConnectionStatus status_;
    void update();
    std::string currentConnectionInfo();

  public:
    NetworkDisplay();
    void assign(Networks &&networks);
    void setState(snm::ConnectionState &&state);
    void setStatus(snm::ConnectionStatus status);
    std::optional<snm::NetworkInfo> selectedNetwork();
    bool pressed(int ch) override;
};
