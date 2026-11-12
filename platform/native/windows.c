// Win32 window tweaks not exposed by Tauri's public JS/Rust API.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

// Forces the window to the top of the z-order without activating it, and
// keeps it there when other topmost windows appear.
void lanchat_win_make_topmost(void *hwnd) {
    if (hwnd == NULL) {
        return;
    }
    SetWindowPos((HWND)hwnd, HWND_TOPMOST, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

// Shows the window with SW_SHOWNOACTIVATE so keyboard focus stays where the
// user is typing.
void lanchat_win_show_without_focus(void *hwnd) {
    if (hwnd == NULL) {
        return;
    }
    ShowWindow((HWND)hwnd, SW_SHOWNOACTIVATE);
    SetWindowPos((HWND)hwnd, HWND_TOPMOST, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
}

// Simple system notification chime (no bundled asset required).
void lanchat_win_beep(void) {
    MessageBeep(MB_ICONASTERISK);
}
