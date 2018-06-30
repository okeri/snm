#include <form.h>

#include <vector>
#include "NetworkProps.hh"

class NetworkProps::Impl {
    WINDOW *win_;
    FORM *form_;
    std::vector<FIELD *> fields_;
    std::string essid_;
    snm::ConnectionProps props_;

    enum Fields : uint32_t {
        AutoConnect,
        Encryption,
        Password
    } sel_;

  public:
    explicit Impl(WINDOW *w) : win_(w) {
        fields_.emplace_back(new_field(1, 16, 5, 15, 0, 0));
        set_field_back(fields_[0], A_UNDERLINE);
        fields_.emplace_back(nullptr);
        form_ = new_form(fields_.data());
        set_form_win(form_, win_);
        set_current_field(form_, fields_[0]);

        int rows, cols;
        scale_form(form_, &rows, &cols);
        set_form_sub(form_, derwin(win_, rows, cols, 0, 0));
        post_form(form_);
    }

    ~Impl() {
        unpost_form(form_);
        free_form(form_);
        free_field(fields_[0]);
    }

    void update() {
        wmove(win_, 1, 2);
        wclrtoeol(win_);

        box(win_, 0, 0);
        bool eth = essid_.empty();
        auto control = ColorControl(win_, Colors::Head);
        wprintw(win_, "Settings for %s",
                eth ? "Ethernet connection" : essid_.c_str());
        control.release();
        if (sel_ == Fields::AutoConnect) {
            control = Colors::Tagged;
        }
        mvwprintw(win_, 3, 2, "auto connect [%c]",
                  props_.auto_connect ? 'X' : ' ');
        control.release();

        if (sel_ == Fields::Encryption) {
            control = Colors::Tagged;
        }
        mvwprintw(win_, 4, 2, "encrypted    [%c]",
                  props_.password ? 'X' : ' ');
        control.release();
        if (sel_ == Fields::Password) {
            control = Colors::Tagged;
        }
        mvwprintw(win_, 5, 2, "password");
        control.release();
        mvwprintw(win_, 7, 8, "<- Cancel      Apply ->");
        update_panels();
        doupdate();
    }

    void assign(std::string_view essid, snm::ConnectionProps &&props) {
        essid_ = essid;
        props_ = props;
        sel_ = Fields::AutoConnect;
        updateProps();
    }

    void updateProps() {
        if (!props_.password) {
            set_field_buffer(fields_[0], 0, "");
            set_field_back(fields_[0], A_INVIS);
        } else {
            set_field_back(fields_[0], A_UNDERLINE);
            set_field_buffer(fields_[0], 0, props_.password.value().c_str());
        }
        update();
    }

    std::pair<std::string, snm::ConnectionProps> get() {
        return std::make_pair(essid_, props_);
    }

    bool pressed(int ch) {
        switch (ch) {
            case KEY_APPLY: case KEY_RIGHT:
                if (props_.password) {
                    form_driver(form_, REQ_VALIDATION);
                    std::string password = field_buffer(fields_[0], 0);
                    password.erase(password.find_last_not_of(" \n") + 1);
                    props_.password = password;
                }

                [[fallthrough]];

            case KEY_LEFT:  case KEY_ESC:
                return false;

            case KEY_UP:
                if (sel_ != Fields::AutoConnect) {
                    sel_ = static_cast<Fields>(static_cast<uint32_t>(sel_) - 1);
                    update();
                }
                break;

            case ' ':
                switch (sel_) {
                    case 0:
                        props_.auto_connect = !props_.auto_connect;
                        update();
                        break;

                    case 1:
                        if (props_.password) {
                            props_.password = std::nullopt;
                        } else {
                            props_.password = "";
                        }
                        updateProps();
                        break;

                    default:
                        form_driver(form_, ch);
                        break;
                }
                break;

            case KEY_DOWN:
                if (sel_ == Fields::AutoConnect ||
                    (sel_ == Fields::Encryption &&
                     props_.password)) {
                    sel_ = static_cast<Fields>(static_cast<uint32_t>(sel_) + 1);
                    update();
                }
                break;

            case KEY_BACKSPACE:
                if (sel_ == Fields::Password) {
                    form_driver(form_, REQ_DEL_PREV);
                }
                break;

            default:
                if (sel_ == Fields::Password) {
                    form_driver(form_, ch);
                }
        }
        update_panels();
        doupdate();
        return true;
    }
};

NetworkProps::NetworkProps() :
        Window(40, 9),
        pImpl_(std::make_unique<Impl>(win_)) {
}

NetworkProps::~NetworkProps() {
}

void NetworkProps::assign(std::string_view essid,
                          snm::ConnectionProps &&props) {
    pImpl_->assign(essid, std::move(props));
}

bool NetworkProps::pressed(int ch) {
    return pImpl_->pressed(ch);
}

std::pair<std::string, snm::ConnectionProps> NetworkProps::get() {
    return pImpl_->get();
}
