use std::{fs::File, io::BufReader};

use dioxus::desktop::use_window;
use dioxus::prelude::*;
use futures_util::StreamExt;
use rodio::{Decoder, OutputStream, Sink};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");

const PATH: &str = r#"/Users/amit/Desktop/Zhea Erose - Dreamsura/"#;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_window().window.set_always_on_top(false);
    let _sound = use_coroutine(play_audio);

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Hero {}

    }
}

#[component]
pub fn Hero() -> Element {
    rsx! {
        div {
            id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus", "💫 VSCode Extension" }
                a { href: "https://discord.gg/XgGxMSkvUM", "👋 Community Discord" }
            }
        }
    }
}

async fn play_audio(mut rx: UnboundedReceiver<()>) {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();

    let file = BufReader::new(File::open("assets/honey.wav").unwrap());
    let source = Decoder::new(file).unwrap();

    sink.append(source);

    let file = BufReader::new(File::open("assets/silver_lullaby.wav").unwrap());
    let source = Decoder::new(file).unwrap();

    sink.append(source);

    while let Some(_message) = rx.next().await {}
}
