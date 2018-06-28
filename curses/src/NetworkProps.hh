#pragma once

#include <memory>
#include <string>
#include <tuple>
#include <string_view>
#include <snm_types.hh>
#include "Window.hh"

class NetworkProps: public Window {
    class Impl;
    std::unique_ptr<Impl> pImpl_;

  public:
    NetworkProps();
    ~NetworkProps();
    void assign(std::string_view essid, snm::ConnectionProps &&props);
    std::tuple<std::string, snm::ConnectionProps> get();

    bool pressed(int ch) override;
};
