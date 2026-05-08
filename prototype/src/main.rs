//! Baudrun · alacritty + gpui prototype.
//!
//! Checkpoint #2: render a 2D grid of cells with per-cell
//! foreground / background colors. Sample content below mimics
//! the kind of output a Cisco IOS session produces — banner +
//! highlighted keywords + an erred-disabled interface — so we can
//! eyeball that the per-cell color routing works without yet
//! plumbing a real VT parser.

mod terminal_grid;

use gpui::{
    px, App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
};

use terminal_grid::{Cell, TerminalGrid};

/// Baudrun palette colors, copied out of `builtin_themes.json` so
/// the prototype's sample looks recognizably like a real session.
mod color {
    pub const BG: u32 = 0x0b0b0d;
    pub const FG: u32 = 0xe4e4e7;
    pub const DIM: u32 = 0x4a4a52;
    pub const RED: u32 = 0xff6961;
    pub const GREEN: u32 = 0x7cd992;
    pub const YELLOW: u32 = 0xf5d76e;
    pub const BLUE: u32 = 0x6cb6ff;
    pub const MAGENTA: u32 = 0xd794ff;
    pub const CYAN: u32 = 0x7ce0e0;
    /// Selection background — used here just to demonstrate per-cell
    /// `bg` working for a highlighted region.
    pub const SELECTION_BG: u32 = 0x1a3a5c;
}

fn populate_sample(grid: &mut TerminalGrid) {
    use color::*;

    // Banner — bright keywords, dim punctuation.
    grid.write_str(0, 0, "Router>", FG, BG);
    grid.write_str(0, 8, "show running-config", CYAN, BG);

    grid.write_str(2, 0, "Building configuration...", DIM, BG);
    grid.write_str(4, 0, "!", DIM, BG);
    grid.write_str(5, 0, "version 15.4", DIM, BG);
    grid.write_str(6, 0, "service timestamps debug datetime msec", DIM, BG);
    grid.write_str(7, 0, "service password-encryption", DIM, BG);
    grid.write_str(8, 0, "!", DIM, BG);

    grid.write_str(10, 0, "interface GigabitEthernet0/1", BLUE, BG);
    grid.write_str(11, 2, "ip address ", FG, BG);
    grid.write_str(11, 13, "10.10.10.1", GREEN, BG);
    grid.write_str(11, 24, " ", FG, BG);
    grid.write_str(11, 25, "255.255.255.0", GREEN, BG);
    grid.write_str(12, 2, "no ip redirects", FG, BG);
    grid.write_str(13, 2, "duplex full", FG, BG);
    grid.write_str(14, 2, "speed 1000", FG, BG);

    grid.write_str(16, 0, "interface GigabitEthernet0/2", BLUE, BG);
    grid.write_str(17, 2, "shutdown", RED, BG);
    grid.write_str(18, 2, "description ", FG, BG);
    grid.write_str(18, 14, "TO-CORE-SW1", YELLOW, BG);

    grid.write_str(20, 0, "% Selection demo:", FG, BG);
    grid.write_str(20, 18, " highlighted region ", FG, SELECTION_BG);
    grid.write_str(20, 38, " end", FG, BG);

    grid.write_str(22, 0, "Router#", MAGENTA, BG);
    grid.set_cell(22, 7, Cell { ch: '_', fg: FG, bg: BG }); // fake cursor
}

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Baudrun (prototype) · checkpoint #2".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| {
                cx.new(|_| {
                    let mut grid = TerminalGrid::new(24, 80, color::FG, color::BG);
                    populate_sample(&mut grid);
                    grid
                })
            },
        )
        .expect("open window");
        cx.activate(true);
    });
}
