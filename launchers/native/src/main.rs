use std::io::Cursor;

use bevy::{prelude::*, window::WindowId, winit::WinitWindows};
use bevy_starfighter::Settings;
use clap::Parser;
use winit::window::Icon;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, value_parser)]
    agent_path: Option<String>,
    #[clap(long, value_parser)]
    agent_asset: Option<String>,
    #[clap(long, value_parser)]
    frameskip: Option<u32>,
    /// Run in headless mode
    #[clap(long)]
    headless: bool,
    #[clap(long)]
    random_ai: bool,
    #[clap(long)]
    fixed_timestep: bool,
    #[clap(long, value_parser)]
    act_interval: Option<u32>,
    #[clap(long, value_parser)]
    ai_act_interval: Option<u32>,
    #[clap(long, value_parser, default_value = "1")]
    players: u32,
    #[clap(long, value_parser, default_value = "5")]
    asteroid_count: u32,
    /// Enable continuous collision detection
    #[clap(long)]
    ccd: bool,
    #[clap(long, value_parser, default_value = "450")]
    respawn_time: u32,
    #[clap(long, value_parser, default_value = "0.3")]
    opponent_stats_multiplier: f32,
    #[clap(long)]
    human_player: bool,
    #[clap(long)]
    physics_debug_render: bool,
    #[clap(long)]
    log_diagnostics: bool,
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
    let mut settings = Settings::default();
    if let Some(frameskip) = args.frameskip {
        settings.frameskip = frameskip;
    }
    settings.fixed_timestep = args.fixed_timestep;
    settings.random_ai = args.random_ai;
    settings.agent_path = args.agent_path.clone();
    settings.headless = args.headless;
    settings.enable_logging = true;
    if let Some(act_interval) = args.act_interval {
        settings.action_interval = act_interval;
    }
    settings.ai_action_interval = args.ai_act_interval;
    settings.players = args.players;
    settings.asteroid_count = args.asteroid_count;
    settings.continuous_collision_detection = args.ccd;
    settings.respawn_time = args.respawn_time;
    settings.opponent_stats_multiplier = args.opponent_stats_multiplier;
    settings.human_player = args.human_player;
    settings.opponent_policy = args.agent_asset;
    settings.physics_debug_render = args.physics_debug_render;
    settings.log_diagnostics = args.log_diagnostics;
    let mut app = bevy_starfighter::app(settings, vec![]);

    info!("Starting launcher: Native");
    if !args.headless {
        app.add_startup_system(set_window_icon);
    }
    app.run();
}
