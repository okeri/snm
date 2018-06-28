#include "Window.hh"

Window::Window(int width, int height) {
    win_ = newwin(height, width,
                  (LINES - height) / 2,
                  (COLS - width) / 2);
    panel_ = new_panel(win_);
}

Window::~Window() {
    delwin(win_);
}

void Window::setTop() {
    top_panel(panel_);
    update_panels();
    doupdate();
}

ColorControl Window::colorControl(std::optional<Colors> clr) {
    if (clr.has_value()) {
        return ColorControl(win_, clr.value());
    }
    return ColorControl(win_);
}
