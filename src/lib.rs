#[cfg(feature = "python")]
pub mod python;

use bevy::app::AppExit;
use bevy::app::ScheduleRunnerSettings;
use bevy::asset::AssetPlugin;
use bevy::prelude::shape::{Circle, Quad};
use bevy::render::mesh::Indices;
use bevy::render::mesh::MeshPlugin;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::Material2dPlugin;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::time::FixedTimestep;
use bevy::{log, prelude::*};
use entity_gym_rs::agent::{self, Agent, AgentOps, Obs};
use heron::prelude::*;
use heron::PhysicsSteps;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

#[cfg(feature = "python")]
use python::Config;

pub const LAUNCHER_TITLE: &str = "Bevy Shell - Template";

#[derive(Default, Debug)]
struct Stats {
    bullets_fired: usize,
    timesteps: usize,
    bullet_hits: usize,
    destroyed_asteroids: usize,
}

#[derive(Clone)]
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
}

impl Settings {
    fn timestep_secs(&self) -> f32 {
        1.0 / self.frame_rate * self.frameskip as f32
    }
}

pub fn base_app(settings: &Settings, agent: Option<Box<dyn Agent>>) -> App {
    let mut main_system = SystemSet::new()
        .with_system(ai)
        .with_system(check_boundary_collision)
        .with_system(spawn_asteroids)
        .with_system(detect_collisions)
        .with_system(expire_bullets)
        .with_system(fighter_actions.after(ai).after(keyboard_events))
        .with_system(cooldowns.after(fighter_actions))
        .with_system(reset.after(cooldowns));
    if settings.fixed_timestep {
        main_system = main_system.with_run_criteria(FixedTimestep::step(
            settings.timestep_secs() as f64,
        ));
    }
    let mut app = App::new();
    app.add_plugin(PhysicsPlugin::default())
        .insert_resource(SmallRng::seed_from_u64(settings.seed))
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(RemainingTime(2700))
        .insert_resource(Stats::default())
        .insert_resource(settings.clone())
        .insert_non_send_resource(Player(agent))
        .add_event::<GameOver>()
        .add_event::<act::FighterAction>()
        .add_system_set(main_system);
    app
}

pub fn app(settings: Settings, agent: Option<Box<dyn Agent>>) -> App {
    let agent: Option<Box<dyn Agent>> = match &settings.agent_path {
        Some(path) => Some(agent::load(path)),
        None if settings.random_ai => Some(agent::random()),
        None => agent,
    };
    let mut app = base_app(&settings, agent);
    if settings.headless {
        app.insert_resource(ScheduleRunnerSettings::run_loop(
            Duration::from_secs_f64(0.0),
        ))
        .insert_resource(PhysicsSteps::every_frame(Duration::from_secs_f64(
            settings.timestep_secs() as f64,
        )))
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
        app.insert_resource(WindowDescriptor {
            title: LAUNCHER_TITLE.to_string(),
            width: 2000.0,
            height: 1000.0,
            canvas: Some("#bevy".to_string()),
            fit_canvas_to_parent: true,
            ..Default::default()
        })
        .insert_resource(PhysicsSteps::every_frame(Duration::from_secs_f64(
            settings.timestep_secs() as f64,
        )))
        .add_system(keyboard_events)
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup);
    }
    app
}

#[cfg(feature = "python")]
pub fn run_training(
    config: Config,
    agent: entity_gym_rs::agent::TrainAgent,
    seed: u64,
) {
    let settings = Settings {
        seed,
        frameskip: config.frameskip,
        action_interval: config.act_interval,
        headless: true,
        ..Settings::default()
    };
    app(settings, Some(Box::new(agent))).run();
}

#[allow(clippy::too_many_arguments)]
fn reset(
    mut game_over: EventReader<GameOver>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<Entity, With<Asteroid>>,
    mut stats: ResMut<Stats>,
    mut fighter: Query<Entity, With<Fighter>>,
    mut remaining_time: ResMut<RemainingTime>,
    mut player: NonSendMut<Player>,
) {
    if let Some(GameOver) = game_over.iter().next() {
        if let Some(p) = player.0.as_mut() {
            p.game_over(
                &Obs::new(stats.destroyed_asteroids as f32)
                    .metric("bullets_fired", stats.bullets_fired as f32)
                    .metric("timesteps", stats.timesteps as f32)
                    .metric("bullet_hits", stats.bullet_hits as f32)
                    .metric(
                        "destroyed_asteroids",
                        stats.destroyed_asteroids as f32,
                    ),
            );
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
        remaining_time.0 = 2700;
        spawn_player(&mut cmd, &mut meshes, &mut materials);
    }
}

fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_player(&mut cmd, &mut meshes, &mut materials);
    cmd.spawn_bundle(Camera2dBundle::default());
    // Spawn rectangular bounds
    let bounds = Quad::new(Vec2::new(2000.0, 1000.0));
    let handle = meshes.add(bounds.into());
    cmd.spawn().insert_bundle(ColorMesh2dBundle {
        mesh: handle.into(),
        transform: Transform::default(),
        material: materials
            .add(ColorMaterial::from(Color::rgb(0.07, 0.07, 0.07))),
        ..default()
    });
}

fn spawn_player(
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    cmd.spawn()
        .insert(Fighter {
            max_velocity: 500.0,
            acceleration: 2000.0,
            turn_speed: 5.0,
            bullet_speed: 1500.0,
            bullet_lifetime: 45,
            bullet_cooldown: 12,
            bullet_kickback: 50.0,
            remaining_bullet_cooldown: 0,
        })
        .insert(RigidBody::Dynamic)
        .insert(PhysicMaterial {
            restitution: 1.0,
            density: 10000.0,
            friction: 0.5,
        })
        .insert(CollisionShape::ConvexHull {
            points: vec![
                50.0 * Vec3::new(0.0, 0.4, 0.0),
                50.0 * Vec3::new(-0.3, -0.4, 0.0),
                50.0 * Vec3::new(0.3, -0.4, 0.0),
            ],
            border_radius: None,
        })
        .insert(Velocity::from_linear(Vec3::new(0.0, 0.0, 0.0)))
        .insert(Acceleration::from_linear(Vec3::new(0.0, 0.0, 0.0)))
        .insert(RotationConstraints::lock())
        .insert(CollisionType::Fighter)
        .insert_bundle(MaterialMesh2dBundle {
            mesh: meshes.add(create_fighter_mesh()).into(),
            transform: Transform::default()
                .with_scale(Vec3::splat(50.0))
                .with_translation(Vec3::new(0.0, 0.0, 1.0)),
            material: materials
                .add(ColorMaterial::from(Color::rgba(0.2, 0.3, 0.6, 1.0))),
            ..default()
        });
}

fn spawn_bullet(
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Vec3,
    velocity: Vec3,
    lifetime: u32,
) {
    let radius = 3.0;
    let circle = Circle::new(radius);
    let handle = meshes.add(circle.into());
    cmd.spawn()
        .insert(Bullet {
            remaining_lifetime: lifetime,
        })
        .insert(RigidBody::Dynamic)
        .insert(PhysicMaterial {
            restitution: 1.0,
            density: 2000.0,
            friction: 0.5,
        })
        .insert(CollisionShape::ConvexHull {
            points: vec![
                5.0 * Vec3::new(0.0, 0.4, 0.0),
                5.0 * Vec3::new(-0.3, -0.4, 0.0),
                5.0 * Vec3::new(0.3, -0.4, 0.0),
            ],
            border_radius: None,
        })
        .insert(Velocity::from_linear(velocity))
        .insert(RotationConstraints::lock())
        .insert(CollisionType::Bullet)
        .insert_bundle(MaterialMesh2dBundle {
            mesh: handle.into(),
            transform: Transform::default()
                .with_scale(Vec3::splat(1.0))
                .with_translation(position),
            material: materials
                .add(ColorMaterial::from(Color::rgb(0.8, 0.8, 0.9))),
            ..default()
        });
}

fn detect_collisions(
    mut cmd: Commands,
    mut events: EventReader<CollisionEvent>,
    collision_type: Query<&CollisionType>,
    mut game_over: EventWriter<GameOver>,
    mut asteroids: Query<(&mut Asteroid, &mut Handle<ColorMaterial>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut stats: ResMut<Stats>,
) {
    for event in events.iter() {
        if let CollisionEvent::Started(data1, data2) = event {
            match (
                collision_type.get(data1.rigid_body_entity()).unwrap(),
                collision_type.get(data2.rigid_body_entity()).unwrap(),
            ) {
                (CollisionType::Fighter, CollisionType::Asteroid) => {
                    cmd.entity(data1.rigid_body_entity()).despawn();
                    game_over.send(GameOver);
                }
                (CollisionType::Asteroid, CollisionType::Fighter) => {
                    cmd.entity(data2.rigid_body_entity()).despawn();
                    game_over.send(GameOver);
                }
                (CollisionType::Bullet, CollisionType::Asteroid) => {
                    cmd.entity(data1.rigid_body_entity()).despawn();
                    let (mut asteroid, mut material) =
                        asteroids.get_mut(data2.rigid_body_entity()).unwrap();
                    asteroid.health -= 1.0;
                    stats.bullet_hits += 1;
                    if asteroid.health <= 0.0 {
                        cmd.entity(data2.rigid_body_entity()).despawn();
                        stats.destroyed_asteroids += 1;
                    } else {
                        *material =
                            materials.add(ColorMaterial::from(Color::rgb(
                                1.0 - 0.08 * asteroid.health,
                                1.0 - 0.1 * asteroid.health,
                                1.0 - 0.1 * asteroid.health,
                            )));
                    }
                }
                (CollisionType::Asteroid, CollisionType::Bullet) => {
                    cmd.entity(data2.rigid_body_entity()).despawn();
                    let (mut asteroid, mut material) =
                        asteroids.get_mut(data1.rigid_body_entity()).unwrap();
                    asteroid.health -= 1.0;
                    stats.bullet_hits += 1;
                    if asteroid.health <= 0.0 {
                        cmd.entity(data1.rigid_body_entity()).despawn();
                        stats.destroyed_asteroids += 1;
                    } else {
                        *material =
                            materials.add(ColorMaterial::from(Color::rgb(
                                1.0 - 0.08 * asteroid.health,
                                1.0 - 0.1 * asteroid.health,
                                1.0 - 0.1 * asteroid.health,
                            )));
                    }
                }
                _ => {}
            }
        }
    }
}

fn check_boundary_collision(
    mut fighter: Query<(&mut Velocity, &Transform, &mut Fighter)>,
) {
    for (mut velocity, transform, fighter) in fighter.iter_mut() {
        let x = transform.translation.x;
        let y = transform.translation.y;
        if x > 1000.0 {
            velocity.linear.x = -fighter.max_velocity;
        } else if x < -1000.0 {
            velocity.linear.x = fighter.max_velocity;
        }
        if y > 500.0 {
            velocity.linear.y = -fighter.max_velocity;
        } else if y < -500.0 {
            velocity.linear.y = fighter.max_velocity;
        }
        // Clamp velocity
        let speed = velocity.linear.length();
        if speed > fighter.max_velocity {
            velocity.linear =
                velocity.linear.normalize() * fighter.max_velocity;
        }
    }
}

fn spawn_asteroids(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut asteroids: Query<(Entity, &mut Asteroid, &mut Transform)>,
    mut rng: ResMut<SmallRng>,
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
    while count < 25 {
        let speed = rng.gen_range(50.0..300.0);
        let direction = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let spawn_angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let size: f32 = rng.gen_range(20.0..60.0) * rng.gen_range(20.0..60.0);
        let circle = Circle::new(size.sqrt());
        let handle = meshes.add(circle.into());
        cmd.spawn()
            .insert(Asteroid {
                health: 5.0,
                radius: size.sqrt(),
            })
            .insert(RigidBody::Dynamic)
            .insert(PhysicMaterial {
                restitution: 1.0,
                density: size,
                friction: 0.5,
            })
            .insert(CollisionShape::Sphere {
                radius: size.sqrt(),
            })
            .insert(Velocity::from_linear(
                speed * Vec3::new(direction.cos(), direction.sin(), 0.0),
            ))
            .insert(RotationConstraints::lock())
            .insert(CollisionType::Asteroid)
            .insert_bundle(ColorMesh2dBundle {
                mesh: handle.into(),
                transform: Transform::default()
                    .with_scale(Vec3::splat(1.0))
                    .with_translation(Vec3::new(
                        1500.0 * spawn_angle.cos(),
                        750.0 * spawn_angle.sin(),
                        1.0,
                    )),
                material: materials
                    .add(ColorMaterial::from(Color::rgba(0.6, 0.5, 0.5, 1.0))),
                ..default()
            });
        count += 1;
    }
}

fn keyboard_events(
    mut action_events: EventWriter<act::FighterAction>,
    remaining_time: Res<RemainingTime>,
    settings: Res<Settings>,
    keys: Res<Input<KeyCode>>,
    // mut key_evr: EventReader<KeyboardInput>,
) {
    if remaining_time.0 as u32 % settings.action_interval != 0 {
        return;
    }
    let thrust = if keys.pressed(KeyCode::Up) || keys.pressed(KeyCode::W) {
        act::Thrust::On
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
    action_events.send(act::FighterAction {
        thrust,
        shoot,
        turn,
    });
}

fn create_fighter_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.4, 0.0], [-0.3, -0.4, 0.0], [0.3, -0.4, 0.0]],
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
    mut cmd: Commands,
    mut bullets: Query<(Entity, &mut Bullet)>,
) {
    for (entity, mut bullet) in &mut bullets.iter_mut() {
        bullet.remaining_lifetime -= 1;
        if bullet.remaining_lifetime == 0 {
            cmd.entity(entity).despawn();
        }
    }
}

fn cooldowns(
    mut fighter: Query<&mut Fighter>,
    mut timer: ResMut<RemainingTime>,
    mut game_over: EventWriter<GameOver>,
    mut stats: ResMut<Stats>,
    settings: Res<Settings>,
) {
    stats.timesteps += settings.frameskip as usize;
    for mut fighter in &mut fighter.iter_mut() {
        fighter.remaining_bullet_cooldown -= settings.frameskip as i32;
    }
    timer.0 -= settings.frameskip as i32;
    if timer.0 <= 0 {
        game_over.send(GameOver);
    }
}

#[allow(clippy::too_many_arguments)]
fn ai(
    mut action_events: EventWriter<act::FighterAction>,
    mut player: NonSendMut<Player>,
    mut fighter: Query<(&mut Fighter, &Transform, &Velocity)>,
    mut exit: EventWriter<AppExit>,
    asteroids: Query<(&Asteroid, &Transform, &Velocity), Without<Fighter>>,
    bullets: Query<(&Bullet, &Transform, &Velocity), Without<Fighter>>,
    remaining_time: Res<RemainingTime>,
    stats: Res<Stats>,
    settings: Res<Settings>,
) {
    if remaining_time.0 as u32 % settings.action_interval != 0 {
        return;
    }
    if let (Some((fighter, transform, velocity)), Some(player)) =
        (fighter.iter_mut().next(), &mut player.0)
    {
        let pos = transform.translation;
        let vel = velocity.linear;
        let (direction_x, direction_y) = transform_to_direction(transform);
        let obs = Obs::new(stats.destroyed_asteroids as f32)
            .entities([entity::Fighter {
                x: pos.x,
                y: pos.y,
                dx: vel.x,
                dy: vel.y,
                direction_x,
                direction_y,
                remaining_time: remaining_time.0,
                gun_cooldown: fighter.remaining_bullet_cooldown.max(0) as u32,
            }])
            .entities(asteroids.iter().map(
                |(asteroid, transform, velocity)| {
                    let pos = transform.translation;
                    let vel = velocity.linear;
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
                let vel = velocity.linear;
                entity::Bullet {
                    x: pos.x,
                    y: pos.y,
                    dx: vel.x,
                    dy: vel.y,
                    lifetime: bullet.remaining_lifetime,
                }
            }));
        let action = player.act::<act::FighterAction>(&obs);
        match action {
            Some(action) => action_events.send(action),
            None => exit.send(AppExit),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fighter_actions(
    mut action_events: EventReader<act::FighterAction>,
    mut cmd: Commands,
    mut fighter: Query<(
        &mut Fighter,
        &Transform,
        &mut Velocity,
        &mut Acceleration,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut stats: ResMut<Stats>,
    remaining_time: Res<RemainingTime>,
    settings: Res<Settings>,
) {
    if remaining_time.0 as u32 % settings.action_interval != 0 {
        return;
    }
    if let Some((mut fighter, transform, mut velocity, mut acceleration)) =
        fighter.iter_mut().next()
    {
        // Reset rotation and acceleration
        velocity.angular = AxisAngle::new(Vec3::Z, 0.0);
        acceleration.linear = Vec3::ZERO;

        for action in action_events.iter() {
            let rot = transform.rotation.xyz();
            let mut angle = transform.rotation.to_axis_angle().1;
            if rot.z < 0.0 {
                angle = -angle;
            }
            let angle2 = angle + std::f32::consts::PI / 2.0;

            match action.turn {
                act::Turn::Left => {
                    velocity.angular =
                        AxisAngle::new(Vec3::Z, fighter.turn_speed);
                }
                act::Turn::Right => {
                    velocity.angular =
                        AxisAngle::new(Vec3::Z, -fighter.turn_speed);
                }
                act::Turn::None => {}
            }
            if let act::Thrust::On = action.thrust {
                let thrust = Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                    * fighter.acceleration;
                acceleration.linear = thrust;
            }

            if let act::Shoot::On = action.shoot {
                if fighter.remaining_bullet_cooldown <= 0 {
                    stats.bullets_fired += 1;
                    spawn_bullet(
                        &mut cmd,
                        &mut meshes,
                        &mut materials,
                        transform.translation,
                        velocity.linear
                            + Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                                * fighter.bullet_speed,
                        fighter.bullet_lifetime,
                    );
                    fighter.remaining_bullet_cooldown =
                        fighter.bullet_cooldown as i32;
                    let kickback = -Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                        * fighter.bullet_kickback;
                    velocity.linear += kickback;
                }
            }
        }
    }
}

#[derive(Component)]
struct Fighter {
    max_velocity: f32,
    acceleration: f32,
    turn_speed: f32,
    bullet_cooldown: u32,
    bullet_speed: f32,
    bullet_lifetime: u32,
    bullet_kickback: f32,
    remaining_bullet_cooldown: i32,
}

#[derive(Component)]
struct Bullet {
    remaining_lifetime: u32,
}

#[derive(Component)]
struct Asteroid {
    health: f32,
    radius: f32,
}

#[derive(Component)]
enum CollisionType {
    Asteroid,
    Fighter,
    Bullet,
}

struct GameOver;

struct RemainingTime(i32);

struct Player(pub Option<Box<dyn Agent>>);

mod entity {
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
    }

    #[derive(Featurizable)]
    pub struct Bullet {
        pub x: f32,
        pub y: f32,
        pub dx: f32,
        pub dy: f32,
        pub lifetime: u32,
    }
}

mod act {
    use entity_gym_rs::agent::Action;

    #[derive(Action, Clone, Copy, Debug)]
    pub struct FighterAction {
        pub thrust: Thrust,
        pub shoot: Shoot,
        pub turn: Turn,
    }

    #[derive(Action, Clone, Copy, Debug)]
    pub enum Thrust {
        On,
        Off,
    }

    #[derive(Action, Clone, Copy, Debug)]
    pub enum Turn {
        Left,
        Right,
        None,
    }

    #[derive(Action, Clone, Copy, Debug)]
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
        }
    }
}
