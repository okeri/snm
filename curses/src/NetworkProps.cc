#include <form.h>
#include <iomanip>
#include <vector>

#include "NetworkProps.hh"

class NetworkProps::Impl {
    WINDOW* win_;
    FORM* form_;
    std::vector<FIELD*> fields_;
    std::string essid_;
    snm::ConnectionProps props_;

    enum Fields : uint32_t {
        First,
        AutoConnect = First,
        Encryption,
        Password,
        Roaming,
        Threshold,
        Last = Threshold
    } sel_;

  public:
    explicit Impl(WINDOW* w) : win_(w) {
        fields_.emplace_back(new_field(1, 16, 5, 15, 1, 0));
        fields_.emplace_back(new_field(1, 3, 7, 15, 0, 0));
        for (auto& field : fields_) {
            set_field_back(field, A_UNDERLINE);
            field_opts_off(field, O_AUTOSKIP);
        }
        fields_.emplace_back(nullptr);
        form_ = new_form(fields_.data());
        set_form_win(form_, win_);

        int rows, cols;
        scale_form(form_, &rows, &cols);
        set_form_sub(form_, derwin(win_, rows, cols, 0, 0));
        post_form(form_);
        sel_ = Fields::First;
    }

    ~Impl() {
        unpost_form(form_);
        free_form(form_);
        free_field(fields_[0]);
    }

    bool allowSelect(Fields field) {
        switch (field) {
            case Fields::Password:
                return props_.password.has_value();

            case Fields::Threshold:
                return props_.threshold.has_value();

            default:
                return true;
        }
    }

    void up() {
        if (sel_ != Fields::First) {
            auto c = sel_;
            do {
                c = static_cast<Fields>(static_cast<uint32_t>(c) - 1);
            } while (c != Fields::First && !allowSelect(c));
            if (allowSelect(c)) {
                sel_ = c;
                update();
            }
        }
    }

    void down() {
        if (sel_ != Fields::Last) {
            auto c = sel_;
            do {
                c = static_cast<Fields>(static_cast<uint32_t>(c) + 1);
            } while (c != Fields::Last && !allowSelect(c));
            if (allowSelect(c)) {
                sel_ = c;
                update();
            }
        }
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
        mvwprintw(
            win_, 3, 2, "auto connect [%c]", props_.auto_connect ? 'X' : ' ');
        control.release();

        if (sel_ == Fields::Encryption) {
            control = Colors::Tagged;
        }
        mvwprintw(win_, 4, 2, "encrypted    [%c]", props_.password ? 'X' : ' ');
        control.release();

        if (sel_ == Fields::Password) {
            control = Colors::Tagged;
            set_current_field(form_, fields_[0]);
        }
        mvwprintw(win_, 5, 2, "password");
        control.release();

        if (sel_ == Fields::Roaming) {
            control = Colors::Tagged;
        }
        mvwprintw(
            win_, 6, 2, "roaming      [%c]", props_.threshold ? 'X' : ' ');
        control.release();

        if (sel_ == Fields::Threshold) {
            set_current_field(form_, fields_[1]);
            control = Colors::Tagged;
        }
        mvwprintw(win_, 7, 2, "threshold");
        control.release();
        mvwprintw(win_, 7, 14, "%c", props_.threshold ? '-' : ' ');
        mvwprintw(win_, 7, 18, "%s", props_.threshold ? "Db" : "  ");
        mvwprintw(win_, 9, 8, "<- Cancel      Apply ->");
        update_panels();
        doupdate();
    }

    void assign(std::string_view essid, snm::ConnectionProps&& props) {
        essid_ = essid;
        props_ = props;
        sel_ = Fields::First;
        updateProps();
    }

    void updateProps() {
        if (!props_.password) {
            set_field_buffer(fields_[0], 0, "");
            set_field_back(fields_[0], A_INVIS);
        } else {
            set_field_back(fields_[0], A_UNDERLINE);
            set_field_buffer(fields_[0], 0, props_.password->c_str());
        }
        if (!props_.threshold) {
            set_field_buffer(fields_[1], 0, "");
            set_field_back(fields_[1], A_INVIS);
        } else {
            set_field_back(fields_[1], A_UNDERLINE);
            std::stringstream tmp;
            tmp << std::setw(3) << -(*props_.threshold);
            set_field_buffer(fields_[1], 0, tmp.str().c_str());
        }
        update();
    }

    std::pair<std::string, snm::ConnectionProps> get() {
        return std::make_pair(essid_, props_);
    }

    bool pressed(int ch) {
        switch (ch) {
            case KEY_APPLY:
            case KEY_RIGHT:
                form_driver(form_, REQ_VALIDATION);
                if (props_.password) {
                    std::string password = field_buffer(fields_[0], 0);
                    password.erase(password.find_last_not_of(" \n") + 1);
                    props_.password = password;
                }
                if (props_.threshold) {
                    std::string to = field_buffer(fields_[1], 0);
                    props_.threshold = -std::stoi(to);
                }

                [[fallthrough]];

            case KEY_LEFT:
            case KEY_ESC:
                return false;

            case KEY_UP:
                up();
                break;

            case ' ':
                switch (sel_) {
                    case Fields::AutoConnect:
                        props_.auto_connect = !props_.auto_connect;
                        update();
                        break;

                    case Fields::Encryption:
                        if (props_.password) {
                            props_.password = std::nullopt;
                        } else {
                            props_.password = "";
                        }
                        updateProps();
                        break;

                    case Fields::Roaming:
                        if (props_.threshold) {
                            props_.threshold = std::nullopt;
                        } else {
                            props_.threshold = -65;
                        }
                        updateProps();
                        break;

                    default:
                        form_driver(form_, ch);
                        break;
                }
                break;

            case KEY_DOWN:
                down();
                break;

            case KEY_BACKSPACE:
                if (sel_ == Fields::Password || sel_ == Fields::Threshold) {
                    form_driver(form_, REQ_DEL_PREV);
                }
                break;

            default:
                switch (sel_) {
                    case Fields::Threshold:
                        if (!isdigit(ch)) {
                            return true;
                        }
                        [[fallthrough]];
                    case Fields::Password:
                        form_driver(form_, ch);
                        break;
                    default:
                        break;
                }
        }
        update_panels();
        doupdate();
        return true;
    }
};

NetworkProps::NetworkProps() :
    Window(40, 11), pImpl_(std::make_unique<Impl>(win_)) {
}

NetworkProps::~NetworkProps() {
}

void NetworkProps::assign(
    std::string_view essid, snm::ConnectionProps&& props) {
    pImpl_->assign(essid, std::move(props));
}

bool NetworkProps::pressed(int ch) {
    return pImpl_->pressed(ch);
}

std::pair<std::string, snm::ConnectionProps> NetworkProps::get() {
    return pImpl_->get();
}
