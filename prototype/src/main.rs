//! Baudrun · alacritty + gpui prototype.
//!
//! First-checkpoint goal: open a gpui window with a single text
//! element that proves the dep graph resolves and the renderer
//! draws something. Everything beyond this lives in follow-up
//! commits — VT parsing wiring, serial port integration, input
//! routing, eventual chrome.

use gpui::{
    div, prelude::*, px, rgb, App, Application, Bounds, Context, IntoElement, Render, Window,
    WindowBounds, WindowOptions,
};

/// Root view for the prototype. Just paints a centered string in
/// the Baudrun color palette so the first checkpoint is "did the
/// window open with text in it?" — not "is the terminal correct?"
struct Prototype;

impl Render for Prototype {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Baudrun's dark-skin foreground / background. Hard-coded
        // for the spike — theme system migration is way out of
        // scope for the first checkpoint.
        let bg = 0x0b0b0d_u32;
        let fg = 0xe4e4e7_u32;
        let accent = 0xd49b3a_u32;

        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .bg(rgb(bg))
            .text_color(rgb(fg))
            .gap_2()
            .child(
                div()
                    .text_2xl()
                    .child("Baudrun · prototype"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(accent))
                    .child("alacritty_terminal + gpui · checkpoint #1"),
            )
            .child(
                div()
                    .text_xs()
                    .opacity(0.6)
                    .child("If you can read this, the dep graph + renderer work."),
            )
    }
}

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, gpui::size(px(1100.0), px(720.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Baudrun (prototype)".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(|_| Prototype),
        )
        .expect("open window");
        cx.activate(true);
    });
}
