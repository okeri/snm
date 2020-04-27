// unfortunately dbusxx-xml2cpp does not covers our needs
// thats why we have to implement dbus interface manually

#pragma once

#include <dbus-c++/dbus.h>

#include "snm_types.hh"
#include <functional>
#include <string>
#include <vector>

namespace snm {

class snm_proxy : public DBus::InterfaceProxy {
    // unmarshalers
    ConnectionState unmarshalConnectionState(DBus::MessageIter& ri) {
        DBus::Struct<uint32_t, std::string, bool, uint32_t, std::string> proxy;
        ri >> proxy;
        return ConnectionState(static_cast<State>(proxy._1), proxy._2, proxy._3,
            proxy._4, proxy._5);
    }

    std::vector<NetworkInfo> unmarshalNetworks(DBus::MessageIter& ri) {
        std::vector<DBus::Struct<uint32_t, std::string, bool, uint32_t>> proxy;
        ri >> proxy;
        std::vector<NetworkInfo> result;
        for (const auto& item : proxy) {
            result.emplace_back(
                static_cast<State>(item._1), item._2, item._3, item._4);
        }
        return result;
    }

    // signal stub
    void state_changed_stub(const ::DBus::SignalMessage& sig) {
        ::DBus::MessageIter ri = sig.reader();
        stateChanged_(unmarshalConnectionState(ri));
    }

    void network_list_stub(const ::DBus::SignalMessage& sig) {
        ::DBus::MessageIter ri = sig.reader();
        networkList_(unmarshalNetworks(ri));
    }
    void status_changed_stub(const ::DBus::SignalMessage& sig) {
        ::DBus::MessageIter ri = sig.reader();
        uint32_t status;
        ri >> status;
        connectionStatusChanged_(static_cast<ConnectionStatus>(status));
    }

  public:
    using StateChanged = std::function<void(ConnectionState&&)>;
    using NetworkList = std::function<void(std::vector<NetworkInfo>&&)>;
    using ConnectionStatusChanged = std::function<void(ConnectionStatus)>;

    snm_proxy(StateChanged sc, ConnectionStatusChanged csc, NetworkList nl) :
        DBus::InterfaceProxy("com.github.okeri.snm"),
        stateChanged_(sc),
        connectionStatusChanged_(csc),
        networkList_(nl) {
        connect_signal(snm_proxy, state_changed, state_changed_stub);
        connect_signal(snm_proxy, network_list, network_list_stub);
        connect_signal(snm_proxy, connect_status_changed, status_changed_stub);
    }

    // methods
    void connect(ConnectionId setting) {
        DBus::Struct<uint32_t, std::string, bool> proxy{
            static_cast<unsigned int>(setting.state), setting.essid,
            setting.enc};
        DBus::CallMessage call;
        DBus::MessageIter wi = call.writer();

        wi << proxy;
        call.member("connect");
        invoke_method_noreply(call);
    }

    void disconnect() {
        DBus::CallMessage call;
        call.member("disconnect");
        invoke_method_noreply(call);
    }

    ConnectionState get_state() {
        DBus::CallMessage call;
        call.member("get_state");
        DBus::Message msg = invoke_method(call);
        DBus::MessageIter ri = msg.reader();
        return unmarshalConnectionState(ri);
    }

    std::vector<NetworkInfo> get_networks() {
        DBus::CallMessage call;
        call.member("get_networks");
        DBus::Message ret = invoke_method(call);
        DBus::MessageIter ri = ret.reader();
        return unmarshalNetworks(ri);
    }

    ConnectionProps get_props(const std::string& essid) {
        DBus::CallMessage call;
        DBus::MessageIter wi = call.writer();

        wi << essid;
        call.member("get_props");
        DBus::Message ret = invoke_method(call);
        DBus::MessageIter ri = ret.reader();

        ConnectionProps result;
        bool enc;
        bool roaming;
        int32_t to;
        std::string password;
        ri >> password;
        ri >> to;
        ri >> result.auto_connect;
        ri >> enc;
        ri >> roaming;
        if (enc) {
            result.password = password;
        }
        if (roaming) {
            result.threshold = to;
        }
        return result;
    }

    void hello() {
        DBus::CallMessage call;
        call.member("hello");
        invoke_method_noreply(call);
    }

    void set_props(const std::string& essid, const ConnectionProps& props) {
        DBus::CallMessage call;
        DBus::MessageIter wi = call.writer();
        wi << essid;
        wi << props.password.value_or("");
        wi << props.threshold.value_or(-65);
        wi << props.auto_connect;
        wi << props.password.has_value();
        wi << props.threshold.has_value();
        call.member("set_props");
        invoke_method_noreply(call);
    }

  private:
    StateChanged stateChanged_;
    ConnectionStatusChanged connectionStatusChanged_;
    NetworkList networkList_;
};

}  // namespace snm
