// Build script.
//
// Currently only does one thing: embed the Windows .exe icon +
// (eventually) Windows manifest into the PE resource section so
// Windows shows the Baudrun icon on the .exe in Explorer, on the
// taskbar, in Alt-Tab, and in the Start menu. cargo-packager's
// `icons` config covers the bundle-level metadata (installer
// branding, Start menu shortcut icon) — this is a separate layer
// the PE itself carries.
//
// `embed_resource::compile` is a no-op on non-Windows targets, so
// this builds cleanly on macOS / Linux too.

fn main() {
    // `compile` returns a status type that's only useful on
    // Windows for failure diagnostics. We bias toward "the build
    // succeeded if the binary linked" — if rc.exe / windres
    // genuinely failed we'd see a link-time error downstream.
    let _ = embed_resource::compile("resources/icon.rc", embed_resource::NONE);
}
