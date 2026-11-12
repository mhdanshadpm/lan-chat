// Native window tweaks that Tauri does not expose directly.
// Compiled by build.rs with -fno-objc-arc and linked against Cocoa.
#import <Cocoa/Cocoa.h>

// NSWindowCollectionBehaviorCanJoinAllSpaces      = 1 << 0
// NSWindowCollectionBehaviorStationary            = 1 << 4
// NSWindowCollectionBehaviorFullScreenAuxiliary   = 1 << 8
//
// The bubble is a floating, non-activating window that follows the user
// across Spaces and can be drawn over fullscreen apps. The argument is the
// window's content NSView, which avoids relying on Tauri's ns_window().
void lanchat_macos_configure_bubble(void *ns_view, int no_focus) {
    if (ns_view == NULL) {
        return;
    }
    NSView *view = (__bridge NSView *)ns_view;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSWindow *window = [view window];
        if (window == nil) {
            return;
        }
        [window setLevel:NSFloatingWindowLevel];
        NSWindowCollectionBehavior behavior =
            NSWindowCollectionBehaviorCanJoinAllSpaces |
            NSWindowCollectionBehaviorStationary |
            NSWindowCollectionBehaviorFullScreenAuxiliary;
        [window setCollectionBehavior:behavior];
        [window setHidesOnDeactivate:NO];
        [window setMovableByWindowBackground:YES];
        [window setReleasedWhenClosed:NO];
        (void)no_focus;
    });
}

// Displays the window above everything without activating the application or
// moving keyboard focus away from the user's current window.
void lanchat_macos_show_without_focus(void *ns_view) {
    if (ns_view == NULL) {
        return;
    }
    NSView *view = (__bridge NSView *)ns_view;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSWindow *window = [view window];
        if (window != nil) {
            [window orderFrontRegardless];
        }
    });
}

// Switches between a regular Dock app and a menu-bar accessory.
void lanchat_macos_set_dock_visible(int visible) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp setActivationPolicy:visible ? NSApplicationActivationPolicyRegular
                                           : NSApplicationActivationPolicyAccessory];
        if (visible) {
            [NSApp activateIgnoringOtherApps:YES];
        }
    });
}
