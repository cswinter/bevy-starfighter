#[cfg(feature = "python")]
pub mod python;

use bevy::app::AppExit;
use bevy::app::ScheduleRunnerSettings;
use bevy::asset::AssetPlugin;
use bevy::core_pipeline::bloom::BloomSettings;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::prelude::shape::{Circle, Quad};
use bevy::render::mesh::Indices;
use bevy::render::mesh::MeshPlugin;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Material2dPlugin;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::time::FixedTimestep;
use bevy::{log, prelude::*};
use bevy_rapier2d::prelude::*;
use entity_gym_rs::agent::{
    self, Agent, AgentOps, Obs, RogueNetAsset, RogueNetAssetLoader,
};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::ops::{Deref, DerefMut};
use std::time::Duration;

#[cfg(feature = "python")]
use python::Config;

pub const LAUNCHER_TITLE: &str = "Bevy Starfighter";

const FIGHTER_COLORS: [Color; 2] = [
    Color::Hsla {
        hue: 250.0,
        saturation: 1.0,
        lightness: 0.85,
        alpha: 1.0,
    },
    Color::Hsla {
        hue: 360.0,
        saturation: 1.0,
        lightness: 0.9,
        alpha: 1.0,
    },
];
const BULLET_COLORS: [Color; 2] =
    [Color::rgb(0.9, 0.9, 1.0), Color::rgb(1.0, 0.7, 0.7)];

#[derive(Resource)]
struct OpponentHandle(Option<Handle<RogueNetAsset>>);

#[derive(Default, Debug, Resource)]
struct Stats {
    bullets_fired: usize,
    timesteps: usize,
    bullet_hits: usize,
    destroyed_asteroids: usize,
    destroyed_opponents: usize,
    destroyed_allies: usize,
}

impl Stats {
    fn player0_score(&self) -> f32 {
        (self.destroyed_asteroids + self.destroyed_opponents) as f32
    }

    fn player1_score(&self) -> f32 {
        10.0 * self.destroyed_allies as f32 - self.timesteps as f32 * 0.001
    }
}

#[derive(Clone, Resource)]
pub struct Settings {
    pub seed: u64,
    pub frameskip: u32,
    pub frame_rate: f32,
    pub fixed_timestep: bool,
    pub random_ai: bool,
    pub agent_path: Option<String>,
    pub headless: bool,
    pub enable_logging: bool,
    pub action_interval: u32,
    pub ai_action_interval: Option<u32>,
    pub players: u32,
    pub asteroid_count: u32,
    pub continuous_collision_detection: bool,
    pub respawn_time: u32,
    pub opponent_stats_multiplier: f32,
    pub max_game_length: u32,
    pub human_player: bool,
    /// The interval at which the number of opponents is increased by one.
    pub difficulty_ramp: u32,
    pub opponent_policy: Option<String>,
    pub physics_debug_render: bool,
    pub log_diagnostics: bool,
    pub disable_bloom: bool,
}

#[derive(Component)]
struct HighscoreText {
    best: u32,
}

impl Settings {
    fn timestep_secs(&self) -> f32 {
        1.0 / self.frame_rate * self.frameskip as f32
    }

    fn ccd(&self) -> Ccd {
        if self.continuous_collision_detection {
            Ccd::enabled()
        } else {
            Ccd::disabled()
        }
    }
}

pub fn base_app(
    settings: &Settings,
    agents: Vec<Option<Box<dyn Agent>>>,
) -> App {
    let mut main_system = SystemSet::new()
        .with_system(ai)
        .with_system(check_boundary_collision)
        .with_system(spawn_asteroids)
        .with_system(detect_collisions)
        .with_system(expire_bullets)
        .with_system(fighter_actions.after(ai).after(keyboard_events))
        .with_system(cooldowns.after(fighter_actions))
        .with_system(respawn.after(cooldowns))
        .with_system(reset.after(respawn));
    if settings.fixed_timestep {
        main_system = main_system.with_run_criteria(FixedTimestep::step(
            settings.timestep_secs() as f64,
        ));
    }
    let timestep_mode = if settings.frameskip > 1 || settings.headless {
        TimestepMode::Fixed {
            dt: 1.0 * settings.frameskip as f32 / settings.frame_rate as f32,
            substeps: 1,
        }
    } else {
        TimestepMode::Variable {
            max_dt: 1.0 / settings.frame_rate,
            time_scale: 1.0,
            substeps: 1,
        }
    };
    let mut app = App::new();
    app.add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
        .insert_resource(RapierConfiguration {
            gravity: Vect::new(0.0, 0.0),
            timestep_mode,
            ..default()
        })
        .insert_resource(OpponentHandle(None))
        .insert_resource(RngState(SmallRng::seed_from_u64(settings.seed)))
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(RemainingTime(settings.max_game_length as i32))
        .insert_resource(Stats::default())
        .insert_resource(settings.clone())
        .insert_non_send_resource(Players(
            agents
                .into_iter()
                .map(|a| Player {
                    agent: a,
                    ids: vec![],
                    respawns: vec![],
                })
                .collect(),
        ))
        .add_event::<GameOver>()
        .add_event::<(act::FighterAction, Entity)>()
        .add_system_set(main_system);
    app
}

pub fn app(settings: Settings, agents: Vec<Box<dyn Agent>>) -> App {
    let mut agents: Vec<Option<Box<dyn Agent>>> =
        agents.into_iter().map(Some).collect();
    if settings.human_player {
        agents.push(None);
    }
    while agents.len() < settings.players as usize {
        let agent = match &settings.agent_path {
            Some(path) => Some(agent::load(path)),
            None => {
                if settings.random_ai {
                    Some(agent::random())
                } else {
                    None
                }
            }
        };
        agents.push(agent);
    }
    let mut app = base_app(&settings, agents);
    if settings.headless {
        app.insert_resource(ScheduleRunnerSettings::run_loop(
            Duration::from_secs_f64(0.0),
        ))
        .add_plugins(MinimalPlugins)
        .add_plugin(AssetPlugin::default())
        .add_plugin(MeshPlugin)
        .add_plugin(MaterialPlugin::<StandardMaterial>::default())
        .add_plugin(Material2dPlugin::<ColorMaterial>::default())
        .add_startup_system(setup);
        if settings.enable_logging {
            app.add_plugin(bevy::log::LogPlugin::default());
        }
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                title: LAUNCHER_TITLE.to_string(),
                width: 2000.0,
                height: 1000.0,
                canvas: Some("#bevy".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            },
            ..default()
        }))
        .add_system(keyboard_events)
        .add_startup_system(setup);
        if settings.physics_debug_render {
            app.add_plugin(RapierDebugRenderPlugin::default());
        }
    }
    if settings.log_diagnostics {
        app.add_plugin(LogDiagnosticsPlugin::default())
            .add_plugin(FrameTimeDiagnosticsPlugin::default());
    }

    app.add_asset::<RogueNetAsset>()
        .init_asset_loader::<RogueNetAssetLoader>()
        .add_system(apply_policy_asset)
        .add_system(update_score)
        .add_startup_system(load_opponent_policy)
        .add_startup_system(spawn_highscore_text);
    app
}

#[cfg(feature = "python")]
pub fn run_training(
    config: Config,
    agents: Vec<entity_gym_rs::agent::TrainAgent>,
    seed: u64,
) {
    let settings = Settings {
        seed,
        frameskip: config.frameskip,
        action_interval: config.act_interval,
        headless: true,
        continuous_collision_detection: config.ccd,
        ..Settings::default()
    };
    app(
        settings,
        agents
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn Agent>)
            .collect(),
    )
    .run();
}

#[cfg(feature = "python")]
pub fn train1(
    config: Config,
    agent: entity_gym_rs::agent::TrainAgent,
    seed: u64,
) {
    run_training(config, vec![agent], seed);
}

#[cfg(feature = "python")]
pub fn train2(
    config: Config,
    agents: [entity_gym_rs::agent::TrainAgent; 2],
    seed: u64,
) {
    let [a1, a2] = agents;
    run_training(config, vec![a1, a2], seed);
}

#[allow(clippy::too_many_arguments)]
fn reset(
    settings: Res<Settings>,
    mut game_over: EventReader<GameOver>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<Entity, With<Asteroid>>,
    mut stats: ResMut<Stats>,
    mut fighter: Query<Entity, With<Fighter>>,
    mut jets: Query<Entity, With<Jet>>,
    mut bullets: Query<Entity, With<Bullet>>,
    mut remaining_time: ResMut<RemainingTime>,
    mut players: NonSendMut<Players>,
) {
    if let Some(GameOver) = game_over.iter().next() {
        for (
            i,
            Player {
                agent,
                ids,
                respawns,
            },
        ) in players.0.iter_mut().enumerate()
        {
            let score = if i == 0 {
                stats.player0_score()
            } else {
                stats.player1_score()
            };
            if let Some(p) = agent {
                p.game_over(
                    &Obs::new(score)
                        .metric("bullets_fired", stats.bullets_fired as f32)
                        .metric("timesteps", stats.timesteps as f32)
                        .metric("bullet_hits", stats.bullet_hits as f32)
                        .metric(
                            "destroyed_asteroids",
                            stats.destroyed_asteroids as f32,
                        )
                        .metric(
                            "destroyed_opponents",
                            stats.destroyed_opponents as f32,
                        )
                        .metric(
                            "destroyed_allies",
                            stats.destroyed_allies as f32,
                        )
                        .metric(&format!("player_{}_score", i), score),
                );
            }
            ids.clear();
            respawns.clear();
        }
        log::info!("Game Over! Stats: {:?}", stats);
        *stats = Stats::default();
        // Despawn all entities
        for entity in query.iter_mut() {
            cmd.entity(entity).despawn_recursive();
        }
        for entity in fighter.iter_mut() {
            cmd.entity(entity).despawn_recursive();
        }
        for entity in jets.iter_mut() {
            cmd.entity(entity).despawn_recursive();
        }
        for entity in bullets.iter_mut() {
            cmd.entity(entity).despawn_recursive();
        }
        remaining_time.0 = settings.max_game_length as i32;
        spawn_players(
            &settings,
            &mut cmd,
            &mut meshes,
            &mut materials,
            &mut players,
        );
    }
}

fn setup(
    settings: Res<Settings>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut players: NonSendMut<Players>,
) {
    spawn_players(
        &settings,
        &mut cmd,
        &mut meshes,
        &mut materials,
        &mut players,
    );
    if settings.physics_debug_render || settings.disable_bloom {
        cmd.spawn(Camera2dBundle::default());
    } else {
        cmd.spawn((
            Camera2dBundle {
                camera: Camera {
                    hdr: true,
                    ..default()
                },
                ..default()
            },
            BloomSettings {
                threshold: 0.6,
                ..default()
            },
        ));
    }
    // Spawn rectangular bounds
    let bounds = Quad::new(Vec2::new(2000.0, 1000.0));
    let handle = meshes.add(bounds.into());
    cmd.spawn(ColorMesh2dBundle {
        mesh: handle.into(),
        transform: Transform::default(),
        material: materials
            .add(ColorMaterial::from(Color::rgb(0.07, 0.07, 0.07))),
        ..default()
    });
}

fn spawn_players(
    settings: &Settings,
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    players: &mut NonSendMut<Players>,
) {
    spawn_fighter(
        &mut players.0[0],
        0,
        settings,
        cmd,
        meshes,
        materials,
        Vec3::new(0.0, 0.0, 0.5),
    )
}

fn spawn_fighter(
    player: &mut Player,
    player_id: usize,
    settings: &Settings,
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Vec3,
) {
    let stats_multiplier = if player_id == 0 {
        1.0
    } else {
        settings.opponent_stats_multiplier
    };
    let collider = if player_id == 0 {
        Collider::convex_hull(&[
            Vect::new(0.0, 0.5),
            Vect::new(-0.8, -0.6),
            Vect::new(0.8, -0.6),
        ])
    } else {
        Collider::convex_hull(&[
            Vect::new(0.0, 0.3),
            Vect::new(-0.3, -0.3),
            Vect::new(0.3, -0.3),
        ])
    };
    // Account for larger mass
    let acceleration_multiplier = if player_id == 0 { 5.0 } else { 1.0 };
    let deceleration_multiplier = if player_id == 0 { 3.0 } else { 1.0 };
    let entity = cmd
        .spawn(Fighter {
            max_velocity: 1000.0 * stats_multiplier,
            acceleration: 1000000.0
                * stats_multiplier
                * acceleration_multiplier,
            deceleration: 1000000.0
                * stats_multiplier
                * deceleration_multiplier,
            drag_exp: 1.5,
            drag_coef: 0.02,
            turn_acceleration: 0.5,
            max_turn_speed: 8.0 * stats_multiplier,
            bullet_speed: 2500.0 * stats_multiplier,
            bullet_lifetime: if player_id == 0 { 24 } else { 150 },
            bullet_cooldown: if player_id == 0 { 24 } else { 72 },
            remaining_bullet_cooldown: 0,
            is_turning: false,
            player_id,
            act_interval: match player.agent {
                Some(_) => settings
                    .ai_action_interval
                    .unwrap_or(settings.action_interval),
                None => settings.action_interval,
            },
            shield_active: player_id == 0,
            shield_cooldown: 0,
            shield_recharge_period: 300,
        })
        .insert(RigidBody::Dynamic)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(collider.unwrap())
        .insert(Velocity {
            linvel: Vec2::new(0.0, 0.0),
            angvel: 0.0,
        })
        .insert(ExternalForce {
            force: Vec2::new(0.0, 0.0),
            torque: 0.0,
        })
        .insert(ExternalImpulse {
            impulse: Vec2::new(0.0, 0.0),
            torque_impulse: 0.0,
        })
        .insert(CollisionType::Fighter)
        .insert(MaterialMesh2dBundle {
            mesh: meshes
                .add(if player_id == 0 {
                    create_fighter_mesh()
                } else {
                    create_fighter_mesh2()
                })
                .into(),
            transform: Transform::default()
                .with_scale(Vec3::splat(50.0))
                .with_translation(Vec3::new(position.x, position.y, 1.0)),
            material: materials.add(FIGHTER_COLORS[player_id].into()),
            ..default()
        })
        .with_children(|parent| {
            let handle = meshes.add(create_jet_mesh());
            parent
                .spawn(ColorMesh2dBundle {
                    mesh: handle.into(),
                    material: materials.add(ColorMaterial::from(Color::rgba(
                        1.0, 1.0, 1.0, 1.0,
                    ))),
                    transform: Transform::default()
                        .with_translation(Vec3::new(0.0, 0.0, 0.0)),
                    ..default()
                })
                .insert(Jet);
            if player_id == 0 {
                let handle = meshes.add(Circle::new(1.0).into());
                parent
                    .spawn(ColorMesh2dBundle {
                        mesh: handle.into(),
                        material: materials.add(ColorMaterial::from(
                            Color::hsla(250.0, 1.0, 0.25, 1.0),
                        )),
                        transform: Transform::default()
                            .with_translation(Vec3::new(0.0, -0.2, -0.00001)),
                        ..default()
                    })
                    .insert(Shield);
            }
        })
        .id();
    player.ids.push(entity);
}

fn respawn(
    settings: Res<Settings>,
    stats: Res<Stats>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut players: NonSendMut<Players>,
    mut rng: ResMut<RngState>,
) {
    for (i, player) in players.0.iter_mut().enumerate() {
        let target_count = if i == 0 {
            1
        } else {
            1 + stats.timesteps / settings.difficulty_ramp as usize
        };
        if player.ids.len() + player.respawns.len() < target_count {
            player.respawns.push(settings.respawn_time as i32);
        }
        for j in 0..player.respawns.len() {
            player.respawns[j] -= settings.frameskip as i32;
            if player.respawns[j] <= 0 {
                let spawn_pos = match rng.gen_range(0..4) {
                    0 => Vec3::new(-1000.0, rng.gen_range(-500.0..500.0), 0.5),
                    1 => Vec3::new(1000.0, rng.gen_range(-500.0..500.0), 0.5),
                    2 => Vec3::new(rng.gen_range(-1000.0..1000.0), -500.0, 0.5),
                    3 => Vec3::new(rng.gen_range(-1000.0..1000.0), 500.0, 0.5),
                    _ => unreachable!(),
                };
                spawn_fighter(
                    player,
                    i,
                    &settings,
                    &mut cmd,
                    &mut meshes,
                    &mut materials,
                    spawn_pos,
                );
                player.respawns.remove(j);
                break; // loop counter is incorrect now, need to break
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_bullet(
    settings: &Settings,
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Vec3,
    velocity: Vec2,
    lifetime: u32,
    player_id: usize,
) {
    let (mesh, collider) = if player_id == 0 {
        (
            meshes.add(create_projectile_mesh()).into(),
            Collider::convex_hull(&[
                Vect::new(-8.0, 30.0),
                Vect::new(-8.0, -30.0),
                Vect::new(8.0, 0.0),
            ])
            .unwrap(),
        )
    } else {
        let radius = 3.0;
        let circle = Circle::new(radius);
        (meshes.add(circle.into()).into(), Collider::ball(3.0))
    };

    cmd.spawn(Bullet {
        remaining_lifetime: lifetime as i32,
        player_id,
    })
    .insert(RigidBody::Dynamic)
    .insert(collider)
    .insert(Velocity {
        linvel: velocity,
        angvel: 0.0,
    })
    .insert(LockedAxes::ROTATION_LOCKED)
    .insert(CollisionType::Bullet)
    .insert(ActiveEvents::COLLISION_EVENTS)
    .insert(settings.ccd())
    .insert(MaterialMesh2dBundle {
        mesh,
        transform: Transform::default()
            .with_scale(Vec3::splat(1.0))
            .with_rotation(Quat::from_rotation_z(
                Vec2::new(1.0, 0.0).angle_between(velocity),
            ))
            .with_translation(position),
        material: materials.add(ColorMaterial::from(
            BULLET_COLORS[player_id % BULLET_COLORS.len()],
        )),
        ..default()
    });
}

#[allow(clippy::too_many_arguments)]
fn detect_collisions(
    mut cmd: Commands,
    mut events: EventReader<CollisionEvent>,
    collision_type: Query<&CollisionType>,
    mut game_over: EventWriter<GameOver>,
    mut asteroids: Query<(&mut Asteroid, &mut Handle<ColorMaterial>)>,
    mut fighters: Query<(&mut Fighter, &Children)>,
    bullets: Query<&Bullet>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut players: NonSendMut<Players>,
    mut stats: ResMut<Stats>,
    mut shield: Query<&mut Visibility, With<Shield>>,
) {
    //log::info!("{}", stats.timesteps);
    for event in events.iter() {
        if let CollisionEvent::Started(data1, data2, _) = *event {
            // log::info!(
            //     "{:?} <-> {:?} collision ({:?}, {:?})",
            //     collision_type.get(data1.rigid_body_entity()).unwrap(),
            //     collision_type.get(data2.rigid_body_entity()).unwrap(),
            //     data1.rigid_body_entity(),
            //     data2.rigid_body_entity()
            // );
            match (
                collision_type.get(data1).unwrap(),
                collision_type.get(data2).unwrap(),
            ) {
                (CollisionType::Fighter, CollisionType::Asteroid) => {
                    take_hit(
                        &mut cmd,
                        &mut stats,
                        &mut game_over,
                        &mut fighters,
                        &mut shield,
                        &mut players,
                        data1,
                    );
                    cmd.entity(data2).despawn();
                }
                (CollisionType::Asteroid, CollisionType::Fighter) => {
                    take_hit(
                        &mut cmd,
                        &mut stats,
                        &mut game_over,
                        &mut fighters,
                        &mut shield,
                        &mut players,
                        data2,
                    );
                    cmd.entity(data1).despawn();
                }
                (CollisionType::Bullet, CollisionType::Asteroid) => {
                    handle_bullet_asteroid_collision(
                        &mut cmd,
                        &mut asteroids,
                        &mut materials,
                        &mut stats,
                        data2,
                        data1,
                    );
                }
                (CollisionType::Asteroid, CollisionType::Bullet) => {
                    handle_bullet_asteroid_collision(
                        &mut cmd,
                        &mut asteroids,
                        &mut materials,
                        &mut stats,
                        data1,
                        data2,
                    );
                }
                (CollisionType::Fighter, CollisionType::Bullet) => {
                    if fighters.get(data1).unwrap().0.player_id
                        != bullets.get(data2).unwrap().player_id
                    {
                        take_hit(
                            &mut cmd,
                            &mut stats,
                            &mut game_over,
                            &mut fighters,
                            &mut shield,
                            &mut players,
                            data1,
                        );
                        cmd.entity(data2).despawn();
                    }
                }
                (CollisionType::Bullet, CollisionType::Fighter) => {
                    if fighters.get(data2).unwrap().0.player_id
                        != bullets.get(data1).unwrap().player_id
                    {
                        take_hit(
                            &mut cmd,
                            &mut stats,
                            &mut game_over,
                            &mut fighters,
                            &mut shield,
                            &mut players,
                            data2,
                        );
                        cmd.entity(data1).despawn();
                    }
                }
                (CollisionType::Bullet, CollisionType::Bullet) => {
                    let bullet1 = bullets.get(data1).unwrap();
                    let bullet2 = bullets.get(data2).unwrap();
                    if bullet1.player_id == 0 {
                        cmd.entity(data2).despawn();
                    }
                    if bullet2.player_id == 0 {
                        cmd.entity(data1).despawn();
                    }
                }
                _ => {}
            }
        }
    }
}

fn handle_bullet_asteroid_collision(
    cmd: &mut Commands,
    asteroids: &mut Query<(&mut Asteroid, &mut Handle<ColorMaterial>)>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    stats: &mut ResMut<Stats>,
    asteroid_entity: Entity,
    bullet: Entity,
) {
    cmd.entity(bullet).despawn();
    let (mut asteroid, mut material) =
        asteroids.get_mut(asteroid_entity).unwrap();
    asteroid.health -= 1.0;
    stats.bullet_hits += 1;
    if asteroid.health <= 0.0 {
        cmd.entity(asteroid_entity).despawn();
        stats.destroyed_asteroids += 1;
    } else {
        *material = materials.add(ColorMaterial::from(Color::rgb(
            0.6 - 0.1 * asteroid.health,
            0.3 - 0.1 * asteroid.health,
            0.3 - 0.1 * asteroid.health,
        )));
    }
}

fn take_hit(
    cmd: &mut Commands,
    stats: &mut ResMut<Stats>,
    game_over: &mut EventWriter<GameOver>,
    fighters: &mut Query<(&mut Fighter, &Children)>,
    shield: &mut Query<&mut Visibility, With<Shield>>,
    players: &mut NonSendMut<Players>,
    fighter: Entity,
) {
    let mut already_destroyed = true;
    let (mut f, children) = fighters.get_mut(fighter).unwrap();
    if f.shield_active {
        f.shield_active = false;
        f.shield_cooldown = f.shield_recharge_period;
        shield
            .get_mut(*children.iter().nth(1).unwrap())
            .unwrap()
            .is_visible = false;
        return;
    }
    for player in players.0.iter_mut() {
        if let Some(index) = player.ids.iter().position(|id| *id == fighter) {
            player.ids.remove(index);
            already_destroyed = false;
            break;
        }
    }
    if !already_destroyed {
        if let Ok(f) = fighters.get_mut(fighter) {
            stats.bullet_hits += 1;
            if f.0.player_id == 0 {
                game_over.send(GameOver);
                stats.destroyed_allies += 1;
            } else {
                stats.destroyed_opponents += 1;
            }
        }
        cmd.entity(fighter).despawn_recursive();
    }
}

fn check_boundary_collision(
    settings: Res<Settings>,
    mut fighter: Query<(&mut Velocity, &Transform, &mut Fighter)>,
) {
    for (mut velocity, transform, fighter) in fighter.iter_mut() {
        let x = transform.translation.x;
        let y = transform.translation.y;
        if x > 1000.0 {
            velocity.linvel.x = -velocity.linvel.x.abs();
        } else if x < -1000.0 {
            velocity.linvel.x = velocity.linvel.x.abs();
        }
        if y > 500.0 {
            velocity.linvel.y = -velocity.linvel.y.abs();
        } else if y < -500.0 {
            velocity.linvel.y = velocity.linvel.y.abs();
        }
        if !fighter.is_turning && velocity.angvel != 0.0 {
            velocity.angvel -= velocity.angvel.signum()
                * fighter.turn_acceleration
                * settings.frameskip as f32;
        }
        // Clamp velocity
        let speed = velocity.linvel.length();
        if speed > fighter.max_velocity {
            velocity.linvel =
                velocity.linvel.normalize() * fighter.max_velocity;
        }
    }
}

fn spawn_asteroids(
    settings: Res<Settings>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut asteroids: Query<(Entity, &mut Asteroid, &mut Transform)>,
    mut rng: ResMut<RngState>,
) {
    let mut count = 0;
    for asteroid in asteroids.iter_mut() {
        // Delete asteroid if it is out of bounds
        if asteroid.2.translation.x > 1500.0
            || asteroid.2.translation.x < -1500.0
            || asteroid.2.translation.y > 750.0
            || asteroid.2.translation.y < -750.0
        {
            cmd.entity(asteroid.0).despawn();
        } else {
            count += 1;
        }
    }
    while count < settings.asteroid_count {
        let speed = rng.gen_range(50.0..300.0);
        let direction = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let spawn_angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let size: f32 = rng.gen_range(20.0..60.0) * rng.gen_range(20.0..60.0);
        let circle = Circle::new(size.sqrt());
        let handle = meshes.add(circle.into());
        cmd.spawn(Asteroid {
            health: 2.0,
            radius: size.sqrt(),
        })
        .insert(RigidBody::Dynamic)
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(Collider::ball(size.sqrt()))
        .insert(Velocity {
            linvel: speed * Vec2::new(direction.cos(), direction.sin()),
            angvel: 0.0,
        })
        .insert(CollisionType::Asteroid)
        .insert(ColorMesh2dBundle {
            mesh: handle.into(),
            transform: Transform::default()
                .with_scale(Vec3::splat(1.0))
                .with_translation(Vec3::new(
                    1500.0 * spawn_angle.cos(),
                    750.0 * spawn_angle.sin(),
                    1.0,
                )),
            material: materials
                .add(ColorMaterial::from(Color::rgba(0.4, 0.1, 0.1, 1.0))),
            ..default()
        });
        count += 1;
    }
}

fn keyboard_events(
    mut action_events: EventWriter<(act::FighterAction, Entity)>,
    remaining_time: Res<RemainingTime>,
    settings: Res<Settings>,
    keys: Res<Input<KeyCode>>,
    players: NonSend<Players>,
) {
    if remaining_time.0 as u32 % settings.action_interval != 0 {
        return;
    }
    let thrust = if keys.pressed(KeyCode::Up) || keys.pressed(KeyCode::W) {
        act::Thrust::On
    } else if keys.pressed(KeyCode::Down) || keys.pressed(KeyCode::S) {
        act::Thrust::Stop
    } else {
        act::Thrust::Off
    };
    let turn = if keys.pressed(KeyCode::Left) || keys.pressed(KeyCode::A) {
        act::Turn::Left
    } else if keys.pressed(KeyCode::Right) || keys.pressed(KeyCode::D) {
        act::Turn::Right
    } else {
        act::Turn::None
    };
    let shoot = if keys.pressed(KeyCode::Space) {
        act::Shoot::On
    } else {
        act::Shoot::Off
    };
    if (thrust != act::Thrust::Off
        || turn != act::Turn::None
        || shoot != act::Shoot::Off
        || players.0[0].agent.is_none())
        && !players.0[0].ids.is_empty()
    {
        action_events.send((
            act::FighterAction {
                thrust,
                turn,
                shoot,
            },
            players.0[0].ids[0],
        ));
    }
}

fn create_fighter_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [0.0, 0.5, 0.0],
            [-0.8, -0.6, 0.0],
            [0.0, -0.4, 0.0],
            [0.8, -0.6, 0.0],
        ],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1, 2, 2, 3, 0])));
    mesh
}

fn create_jet_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-0.8, -0.6, 0.0],
            [0.0, -0.5, 0.0],
            [0.0, -0.4, 0.0],
            [0.8, -0.6, 0.0],
        ],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1, 2, 1, 3, 2])));
    mesh
}

fn create_projectile_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-8.0, 30.0, 0.0], [-8.0, -30.0, 0.0], [8.0, 0.0, 0.0]],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1, 2])));
    mesh
}

fn create_fighter_mesh2() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.3, 0.0], [-0.3, -0.3, 0.0], [0.3, -0.3, 0.0]],
    );
    mesh.set_indices(Some(Indices::U32(vec![0, 1, 2])));
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 3]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.5, 0.0], [0.0, 1.0], [1.0, 1.0]],
    );

    mesh
}

fn expire_bullets(
    settings: Res<Settings>,
    mut cmd: Commands,
    mut bullets: Query<(Entity, &mut Bullet)>,
) {
    for (entity, mut bullet) in &mut bullets.iter_mut() {
        bullet.remaining_lifetime -= settings.frameskip as i32;
        if bullet.remaining_lifetime <= 0 {
            cmd.entity(entity).despawn();
        }
    }
}

fn cooldowns(
    mut fighter: Query<(&mut Fighter, &Children)>,
    mut timer: ResMut<RemainingTime>,
    mut game_over: EventWriter<GameOver>,
    mut stats: ResMut<Stats>,
    mut shield: Query<&mut Visibility, With<Shield>>,
    settings: Res<Settings>,
) {
    stats.timesteps += settings.frameskip as usize;
    for (mut fighter, children) in &mut fighter.iter_mut() {
        fighter.remaining_bullet_cooldown -= settings.frameskip as i32;
        if !fighter.shield_active && fighter.player_id == 0 {
            fighter.shield_cooldown -= settings.frameskip as i32;
            if fighter.shield_cooldown <= 0 {
                fighter.shield_active = true;
                shield
                    .get_mut(*children.iter().nth(1).unwrap())
                    .unwrap()
                    .is_visible = true;
            }
        }
    }
    timer.0 -= settings.frameskip as i32;
    if timer.0 <= 0 {
        game_over.send(GameOver);
    }
}

#[allow(clippy::too_many_arguments)]
fn ai(
    mut action_events: EventWriter<(act::FighterAction, Entity)>,
    mut players: NonSendMut<Players>,
    fighter: Query<(&mut Fighter, &Transform, &Velocity)>,
    mut exit: EventWriter<AppExit>,
    asteroids: Query<(&Asteroid, &Transform, &Velocity), Without<Fighter>>,
    bullets: Query<(&Bullet, &Transform, &Velocity), Without<Fighter>>,
    remaining_time: Res<RemainingTime>,
    stats: Res<Stats>,
    settings: Res<Settings>,
) {
    let action_interval = settings
        .ai_action_interval
        .unwrap_or(settings.action_interval);
    if remaining_time.0 as u32 % action_interval != 0 {
        return;
    }
    let mut actions = vec![];
    let num_players = players.0.len();
    for (i, agent, ids) in players.0.iter_mut().enumerate().filter_map(
        |(i, Player { agent, ids, .. })| agent.as_mut().map(|a| (i, a, ids)),
    ) {
        if num_players == 1 && ids.is_empty() {
            return;
        }
        let mut actor_entities = vec![];
        let mut xdir = 0.0;
        let mut ydir = 0.0;
        for id in &*ids {
            if let Ok((fighter, transform, velocity)) = fighter.get(*id) {
                let pos = transform.translation;
                let vel = velocity.linvel;
                let (direction_x, direction_y) =
                    transform_to_direction(transform);
                actor_entities.push(entity::Fighter {
                    x: pos.x,
                    y: pos.y,
                    dx: vel.x,
                    dy: vel.y,
                    direction_x,
                    direction_y,
                    remaining_time: remaining_time.0,
                    gun_cooldown: fighter.remaining_bullet_cooldown.max(0)
                        as u32,
                    player: i as u32,
                    shield_active: fighter.shield_active,
                    shield_cooldown: fighter.shield_cooldown as f32
                        / fighter.shield_recharge_period as f32,
                });
                xdir = direction_x;
                ydir = direction_y;
            }
        }
        // Rotates position/direction to align with direction of player 0
        let rotate_x = |x: f32, y: f32| xdir * x + ydir * y;
        let rotate_y = |x: f32, y: f32| -ydir * x + xdir * y;
        let score = if i == 0 {
            stats.player0_score()
        } else {
            stats.player1_score()
        };
        let obs = Obs::new(score)
            .actors(actor_entities)
            .entities(fighter.iter().filter(|(f, _, _)| f.player_id != i).map(
                |(fighter, transform, velocity)| {
                    let pos = transform.translation;
                    let vel = velocity.linvel;
                    let (direction_x, direction_y) =
                        transform_to_direction(transform);
                    entity::EnemyFighter {
                        x: pos.x,
                        y: pos.y,
                        dx: vel.x,
                        dy: vel.y,
                        direction_x,
                        direction_y,
                        gun_cooldown: fighter.remaining_bullet_cooldown.max(0)
                            as u32,
                        player: i as u32,

                        reldx: rotate_x(vel.x, vel.y),
                        reldy: rotate_y(vel.x, vel.y),
                        reldirection_x: rotate_x(direction_x, direction_y),
                        reldirection_y: rotate_y(direction_x, direction_y),
                    }
                },
            ))
            .entities(asteroids.iter().map(
                |(asteroid, transform, velocity)| {
                    let pos = transform.translation;
                    let vel = velocity.linvel;
                    entity::Asteroid {
                        health: asteroid.health,
                        radius: asteroid.radius,
                        x: pos.x,
                        y: pos.y,
                        dx: vel.x,
                        dy: vel.y,
                    }
                },
            ))
            .entities(bullets.iter().map(|(bullet, transform, velocity)| {
                let pos = transform.translation;
                let vel = velocity.linvel;
                entity::Bullet {
                    x: pos.x,
                    y: pos.y,
                    dx: vel.x,
                    dy: vel.y,
                    lifetime: bullet.remaining_lifetime,
                    player: bullet.player_id as u32,
                    reldx: rotate_x(vel.x, vel.y),
                    reldy: rotate_y(vel.x, vel.y),
                }
            }));
        let action = agent.act_async::<act::FighterAction>(&obs);
        actions.push((action, ids.clone()));
    }
    for (action, ids) in actions {
        let action = action.rcv();
        match action {
            Some(actions) => {
                for (action, id) in actions.into_iter().zip(ids) {
                    action_events.send((action, id));
                }
            }
            None => exit.send(AppExit),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fighter_actions(
    mut action_events: EventReader<(act::FighterAction, Entity)>,
    mut cmd: Commands,
    mut fighter: Query<(
        &mut Fighter,
        &Transform,
        &mut Velocity,
        &mut ExternalImpulse,
        &mut ExternalForce,
        &Children,
    )>,
    mut jet: Query<&mut Visibility, With<Jet>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut stats: ResMut<Stats>,
    settings: Res<Settings>,
) {
    for (action, id) in action_events.iter() {
        if let Ok((
            mut fighter,
            transform,
            mut vel,
            mut imp,
            mut force,
            children,
        )) = fighter.get_mut(*id)
        {
            // Reset rotation and acceleration
            imp.impulse = Vec2::new(0.0, 0.0);
            force.force = Vec2::new(0.0, 0.0);
            force.torque = 0.0;
            fighter.is_turning = action.turn != act::Turn::None;

            let rot = transform.rotation.xyz();
            let mut angle = transform.rotation.to_axis_angle().1;
            if rot.z < 0.0 {
                angle = -angle;
            }
            let angle2 = angle + std::f32::consts::PI / 2.0;

            vel.angvel += match action.turn {
                act::Turn::Left => fighter.turn_acceleration,
                act::Turn::QuarterLeft => fighter.turn_acceleration * 0.25,
                act::Turn::Right => -fighter.turn_acceleration,
                act::Turn::QuarterRight => -fighter.turn_acceleration * 0.25,
                act::Turn::None => 0.0,
            } * settings.frameskip as f32;
            vel.angvel = vel
                .angvel
                .clamp(-fighter.max_turn_speed, fighter.max_turn_speed);
            let mut jet_visible = fighter.player_id == 0;
            let speed = vel.linvel.length();
            match action.thrust {
                act::Thrust::On => {
                    let thrust = Vec2::new(angle2.cos(), angle2.sin())
                        * fighter.acceleration;
                    force.force = thrust;
                }
                act::Thrust::Off => {
                    // Should integrate here rather than just multiplying by interval, whatever
                    vel.linvel *= 1.0
                        - fighter.drag_coef
                            * (speed / fighter.max_velocity)
                                .powf(fighter.drag_exp)
                            * fighter.act_interval as f32;
                    jet_visible = false;
                }
                act::Thrust::Stop => {
                    if speed < 1.0 {
                        vel.linvel = Vec2::ZERO;
                    } else {
                        force.force =
                            -vel.linvel.normalize() * fighter.deceleration;
                    }
                }
            }
            jet.get_mut(*children.first().unwrap()).unwrap().is_visible =
                jet_visible;

            if let act::Shoot::On = action.shoot {
                if fighter.remaining_bullet_cooldown <= 0 {
                    stats.bullets_fired += 1;
                    spawn_bullet(
                        &settings,
                        &mut cmd,
                        &mut meshes,
                        &mut materials,
                        transform.translation
                            + 24.0
                                * Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                                * if fighter.player_id == 0 {
                                    -0.0
                                } else {
                                    1.0
                                },
                        vel.linvel
                            + Vec2::new(angle2.cos(), angle2.sin())
                                * fighter.bullet_speed,
                        fighter.bullet_lifetime,
                        fighter.player_id,
                    );
                    fighter.remaining_bullet_cooldown =
                        fighter.bullet_cooldown as i32;
                }
            }
        }
    }
}

fn load_opponent_policy(
    settings: Res<Settings>,
    mut opponent_handles: ResMut<OpponentHandle>,
    server: Res<AssetServer>,
) {
    opponent_handles.0 = settings
        .opponent_policy
        .as_ref()
        .map(|name| server.load(&format!("policies/{}.roguenet", name)));
}

fn apply_policy_asset(
    settings: Res<Settings>,
    mut players: NonSendMut<Players>,
    opponent_handle: Res<OpponentHandle>,
    assets: Res<Assets<RogueNetAsset>>,
) {
    if players.0.last().unwrap().agent.is_none() {
        if let Some(asset) =
            opponent_handle.0.as_ref().and_then(|h| assets.get(h))
        {
            for (i, player) in players.0.iter_mut().enumerate() {
                if i > 0 || !settings.human_player {
                    player.agent = Some(Box::new(asset.agent.clone()));
                }
            }
        }
    }
}

fn spawn_highscore_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let font = asset_server.load("fonts/FiraSans-Bold.ttf");
    let text_style = TextStyle {
        font,
        font_size: 60.0,
        color: Color::WHITE,
    };
    let text_alignment = TextAlignment {
        vertical: VerticalAlign::Top,
        horizontal: HorizontalAlign::Left,
    };
    commands
        .spawn(Text2dBundle {
            text: Text::from_section("Score: 0\nBest: 0", text_style)
                .with_alignment(text_alignment),
            transform: Transform::from_translation(Vec3::new(
                -1000.0, 500.0, 0.3,
            )),
            ..default()
        })
        .insert(HighscoreText { best: 0 });
}

fn update_score(
    stats: Res<Stats>,
    mut highscore_text: Query<(&mut HighscoreText, &mut Text)>,
) {
    if let Some((mut highscore, mut text)) = highscore_text.iter_mut().next() {
        let score = stats.player0_score() as u32;
        highscore.best = highscore.best.max(score);
        text.sections[0].value =
            format!("Score: {}\nBest: {}", score, highscore.best);
    }
}

#[derive(Component)]
struct Fighter {
    max_velocity: f32,
    acceleration: f32,
    deceleration: f32,
    drag_exp: f32,
    drag_coef: f32,
    turn_acceleration: f32,
    max_turn_speed: f32,
    bullet_cooldown: u32,
    bullet_speed: f32,
    bullet_lifetime: u32,
    remaining_bullet_cooldown: i32,
    player_id: usize,
    act_interval: u32,
    is_turning: bool,
    shield_active: bool,
    shield_cooldown: i32,
    shield_recharge_period: i32,
}

#[derive(Component)]
struct Jet;

#[derive(Component)]
struct Shield;

#[derive(Component)]
struct Bullet {
    remaining_lifetime: i32,
    player_id: usize,
}

#[derive(Component)]
struct Asteroid {
    health: f32,
    radius: f32,
}

#[derive(Component, Debug)]
enum CollisionType {
    Asteroid,
    Fighter,
    Bullet,
}

struct GameOver;

#[derive(Resource)]
struct RemainingTime(i32);

#[derive(Resource)]
struct RngState(SmallRng);

#[derive(Debug)]
struct Players(Vec<Player>);

struct Player {
    agent: Option<Box<dyn Agent>>,
    ids: Vec<Entity>,
    respawns: Vec<i32>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("ids", &self.ids)
            .field("respawns", &self.respawns)
            .finish()
    }
}

pub mod entity {
    use entity_gym_rs::agent::Featurizable;

    #[derive(Featurizable)]
    pub struct Asteroid {
        pub health: f32,
        pub radius: f32,
        pub x: f32,
        pub y: f32,
        pub dx: f32,
        pub dy: f32,
    }

    #[derive(Featurizable)]
    pub struct Fighter {
        pub x: f32,
        pub y: f32,
        pub dx: f32,
        pub dy: f32,
        pub direction_x: f32,
        pub direction_y: f32,
        pub remaining_time: i32,
        pub gun_cooldown: u32,
        pub player: u32,
        pub shield_active: bool,
        pub shield_cooldown: f32,
    }

    #[derive(Featurizable)]
    pub struct EnemyFighter {
        pub x: f32,
        pub y: f32,
        pub dx: f32,
        pub dy: f32,
        pub direction_x: f32,
        pub direction_y: f32,
        pub gun_cooldown: u32,
        pub player: u32,

        pub reldx: f32,
        pub reldy: f32,
        pub reldirection_x: f32,
        pub reldirection_y: f32,
    }

    #[derive(Featurizable)]
    pub struct Bullet {
        pub x: f32,
        pub y: f32,
        pub dx: f32,
        pub dy: f32,
        pub lifetime: i32,
        pub player: u32,

        pub reldx: f32,
        pub reldy: f32,
    }
}

pub mod act {
    use entity_gym_rs::agent::Action;

    #[derive(Action, Clone, Copy, Debug)]
    pub struct FighterAction {
        pub thrust: Thrust,
        pub shoot: Shoot,
        pub turn: Turn,
    }

    #[derive(Action, Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Thrust {
        On,
        Off,
        Stop,
    }

    #[derive(Action, Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Turn {
        Left,
        QuarterLeft,
        Right,
        QuarterRight,
        None,
    }

    #[derive(Action, Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Shoot {
        On,
        Off,
    }
}

fn transform_to_direction(transform: &Transform) -> (f32, f32) {
    let rot = transform.rotation.xyz();
    let mut angle = transform.rotation.to_axis_angle().1;
    if rot.z < 0.0 {
        angle = -angle;
    }
    let angle2 = angle + std::f32::consts::PI / 2.0;
    (angle2.cos(), angle2.sin())
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            seed: 0,
            frame_rate: 90.0,
            frameskip: 1,
            fixed_timestep: false,
            random_ai: false,
            agent_path: None,
            headless: false,
            enable_logging: false,
            action_interval: 1,
            players: 1,
            asteroid_count: 5,
            continuous_collision_detection: false,
            respawn_time: 5 * 90, // 5 seconds
            opponent_stats_multiplier: 0.3,
            max_game_length: 2 * 60 * 90, // 2 minutes
            human_player: false,
            difficulty_ramp: 10 * 90, // 10 seconds
            ai_action_interval: None,
            opponent_policy: None,
            physics_debug_render: false,
            log_diagnostics: false,
            disable_bloom: false,
        }
    }
}

impl Deref for RngState {
    type Target = SmallRng;

    fn deref(&self) -> &SmallRng {
        &self.0
    }
}

impl DerefMut for RngState {
    fn deref_mut(&mut self) -> &mut SmallRng {
        &mut self.0
    }
}
