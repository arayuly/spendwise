mod api;
mod app;
mod charts;
mod components;
mod pages;
mod utils;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
