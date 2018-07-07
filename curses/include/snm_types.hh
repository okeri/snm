#pragma once

#include <cstdint>
#include <string>
#include <optional>

namespace snm {

enum class ConnectionStatus {
    Initializing,
    Connecting,
    Authenticating,
    GettingIP,
    AuthFail,
    Aborted,
    ConnectFail,
};

enum class State {
    NotConnected,
    Ethernet,
    Wifi,
    ConnectingEth,
    ConnectingWifi
};

struct NetworkInfo {
    State state;
    std::string essid;
    bool enc;
    uint32_t quality = 0;

    NetworkInfo(State s, const std::string& id, bool e, uint32_t q) noexcept :
            state(s), essid(id), enc(e), quality(q) {
    }

    bool operator==(const NetworkInfo &rhs) const {
        return rhs.state == state &&
                rhs.essid == essid &&
                rhs.enc == enc &&
                rhs.quality == quality;
    }

    bool operator!=(const NetworkInfo &rhs) const {
        return !(*this == rhs);
    }
};

struct ConnectionId {
    State state;
    std::string essid;
    bool enc;

    ConnectionId() noexcept :
            state(State::Ethernet), essid(""), enc(false) {
    }

    ConnectionId(const std::string &id, bool e) noexcept :
            state(State::Wifi), essid(id), enc(e) {
    }

    explicit ConnectionId(const NetworkInfo& info) noexcept :
            state(info.state), essid(info.essid), enc(info.enc) {
    }
};

struct ConnectionState: public NetworkInfo {
    std::string ip;

    ConnectionState(State s, const std::string& id, bool e, uint32_t q,
                    const std::string& i) noexcept :
            NetworkInfo(s, id, e, q), ip(i) {
    }

    bool operator==(const ConnectionState &rhs) const {
        return NetworkInfo::operator == (rhs) &&
                rhs.ip == ip;
    }

    bool operator!=(const ConnectionState &rhs) const {
        return !(*this == rhs);
    }

    ConnectionState &operator=(const ConnectionState &rhs) {
        state = rhs.state;
        essid = rhs.essid;
        enc = rhs.enc;
        quality = rhs.quality;
        ip = rhs.ip;
        return *this;
    }
};

struct ConnectionProps {
    bool auto_connect;
    std::optional<std::string> password;
};

}  // namespace snm
