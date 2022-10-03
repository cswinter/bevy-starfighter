use std::io::Cursor;

use bevy::{prelude::*, window::WindowId, winit::WinitWindows};
use bevy_dogfight_ai::Settings;
use clap::Parser;
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
    #[clap(long, value_parser)]
    ai_act_interval: Option<u32>,
    #[clap(long, value_parser, default_value = "1")]
    players: u32,
    #[clap(long, value_parser, default_value = "25")]
    asteroid_count: u32,
    /// Enable continuous collision detection
    #[clap(long)]
    ccd: bool,
    #[clap(long, value_parser, default_value = "450")]
    respawn_time: u32,
    #[clap(long, value_parser, default_value = "0.6")]
    opponent_stats_multiplier: f32,
    #[clap(long)]
    human_player: bool,
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
        seed: 0,
        frame_rate: 90.0,
        frameskip: args.frameskip,
        fixed_timestep: args.fixed_timestep,
        random_ai: args.random_ai,
        agent_path: args.agent_path.clone(),
        headless: args.headless,
        enable_logging: true,
        action_interval: args.act_interval,
        ai_action_interval: args.ai_act_interval,
        players: args.players,
        asteroid_count: args.asteroid_count,
        continuous_collision_detection: args.ccd,
        respawn_time: args.respawn_time,
        opponent_stats_multiplier: args.opponent_stats_multiplier,
        max_game_length: 2 * 60 * 90, // 2 minutes
        human_player: args.human_player,
        difficulty_ramp: 20 * 90,
    };
    let mut app = bevy_dogfight_ai::app(settings, vec![]);

    info!("Starting launcher: Native");
    if !args.headless {
        app.add_startup_system(set_window_icon);
    }
    app.run();
}
