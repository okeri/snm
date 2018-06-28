#pragma once

#include <panel.h>

#include "ColorControl.hh"

#define KEY_APPLY    10
#define KEY_ESC      27

class Window {
    PANEL *panel_;

  protected:
    WINDOW *win_;

  public:
    Window(int width, int height);
    ~Window();
    void setTop();
    ColorControl colorControl(std::optional<Colors> = std::nullopt);
    virtual bool pressed(int ch) = 0;
};
