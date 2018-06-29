#pragma once

#include <curses.h>
#include <optional>

enum Colors: NCURSES_PAIRS_T {
    Selected = 1,
    Tagged,
    SelTagged,
    Head
};

class ColorControl {
    std::optional<Colors> color_;
    WINDOW *win_;

  public:
    explicit ColorControl(WINDOW* w) : win_(w) {
    }

    ColorControl(WINDOW *w, Colors color): color_(color), win_(w) {
        wattron(win_, COLOR_PAIR(color_.value()));
    }

    ColorControl& operator=(Colors color) {
        if (color_) {
            throw std::logic_error("Second assignment to non-mutable color");
        }
        color_ = color;
        wattron(win_, COLOR_PAIR(color_.value()));
        return *this;
    }

    void release() {
        if (color_) {
            wattroff(win_, COLOR_PAIR(color_.value()));
            color_ = std::nullopt;
        }
    }

    ~ColorControl() {
        if (color_) {
            wattroff(win_, COLOR_PAIR(color_.value()));
        }
    }
};
