// ostheme_darwin.m — Objective-C implementation for the macOS OS-theme
// watcher. Kept in a separate .m file (rather than inline in the CGo
// preamble) because CGo compiles the preamble into multiple translation
// units, which causes duplicate-symbol link errors for @interface/
// @implementation declarations. Moving the class here means cgo
// compiles it exactly once via its auto-compile of .m files in the
// package directory.

#import <Foundation/Foundation.h>

// osthemeGoPoke is defined on the Go side via //export. Forward-declare
// here so the Objective-C compiler accepts the call. It must stay
// trivial — no Go-to-C nesting, no blocking — because it runs inside
// the NSDistributedNotificationCenter dispatch context on the main
// thread. The Go side just pokes a channel; a watcher goroutine picks
// up the work from there.
extern void osthemeGoPoke(void);

@interface OSThemeObserver : NSObject
@end

@implementation OSThemeObserver
- (void)themeChanged:(NSNotification *)n {
    osthemeGoPoke();
}
@end

static OSThemeObserver *gOSThemeObserver = nil;

// Reads the OS-level appearance preference directly from NSUserDefaults,
// bypassing NSApp.effectiveAppearance (which reflects the app's pinned
// NSAppearance, not the system setting).
//
// AppleInterfaceStyle has the quirk of being set to the string "Dark"
// when the OS is in dark mode, and being absent entirely (nil) when in
// light mode — no "Light" value to check for.
int osthemeCurrentIsDark(void) {
    NSString *style = [[NSUserDefaults standardUserDefaults]
                        stringForKey:@"AppleInterfaceStyle"];
    return (style != nil &&
            [style caseInsensitiveCompare:@"Dark"] == NSOrderedSame) ? 1 : 0;
}

// AppleInterfaceThemeChangedNotification is posted on the distributed
// notification center when the user toggles system dark mode. Works
// without entitlements for non-sandboxed apps.
void osthemeStartWatch(void) {
    if (gOSThemeObserver != nil) return;
    gOSThemeObserver = [[OSThemeObserver alloc] init];
    [[NSDistributedNotificationCenter defaultCenter]
        addObserver:gOSThemeObserver
           selector:@selector(themeChanged:)
               name:@"AppleInterfaceThemeChangedNotification"
             object:nil];
}

void osthemeStopWatch(void) {
    if (gOSThemeObserver == nil) return;
    [[NSDistributedNotificationCenter defaultCenter]
        removeObserver:gOSThemeObserver];
    [gOSThemeObserver release];
    gOSThemeObserver = nil;
}
