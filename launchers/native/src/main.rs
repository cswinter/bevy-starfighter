use std::io::Cursor;

use bevy::{prelude::*, window::WindowId, winit::WinitWindows};
use bevy_dogfight_ai::Settings;
use clap::Parser;
use entity_gym_rs::agent;
use winit::window::Icon;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, value_parser)]
    agent_path: Option<String>,
    #[clap(long, value_parser, default_value = "1")]
    frameskip: u32,
    /// Run in headless mode
    #[clap(long)]
    headless: bool,
    #[clap(long)]
    random_ai: bool,
    #[clap(long)]
    fixed_timestep: bool,
    #[clap(long, value_parser, default_value = "1")]
    act_interval: u32,
}

fn set_window_icon(windows: NonSend<WinitWindows>) {
    let primary = windows.get_window(WindowId::primary()).unwrap();

    let (icon_rgba, icon_width, icon_height) = {
        let icon_buf = Cursor::new(include_bytes!("../../../assets/bevy.png"));
        let rgba = image::load(icon_buf, image::ImageFormat::Png)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        let icon_raw = rgba.into_raw();
        (icon_raw, width, height)
    };

    let icon = Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap();

    primary.set_window_icon(Some(icon));
}

fn main() {
    let args = Args::parse();
    let settings = Settings {
        frame_rate: 90.0,
        frameskip: args.frameskip,
        fixed_timestep: args.fixed_timestep,
        random_ai: args.random_ai,
        agent_path: args.agent_path.clone(),
        headless: args.headless,
        enable_logging: true,
        action_interval: args.act_interval,
    };
    let agent: Option<Box<dyn agent::Agent>> = args.agent_path.map(agent::load);
    let mut app = bevy_dogfight_ai::app(settings, agent);

    info!("Starting launcher: Native");
    if !args.headless {
        app.add_startup_system(set_window_icon);
    }
    app.run();
}
