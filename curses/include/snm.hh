#pragma once

#include "snm_proxy.hh"

namespace snm {

class NetworkManager : public snm_proxy,
                       public DBus::IntrospectableProxy,
                       public DBus::ObjectProxy {
  public:
    NetworkManager(DBus::Connection connection, snm_proxy::StateChanged sc,
        snm_proxy::ConnectionStatusChanged csc, snm_proxy::NetworkList nl) :
        snm_proxy(sc, csc, nl),
        DBus::ObjectProxy(connection, "/", "com.github.okeri.snm") {
        hello();
    }
};

}  // namespace snm
