#[cfg(feature = "python")]
pub mod python;

use bevy::app::AppExit;
use bevy::prelude::shape::{Circle, Quad};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::time::FixedTimestep;
use entity_gym_rs::agent::{self, Action, Agent, AgentOps, Featurizable, Obs};
use heron::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

#[cfg(feature = "python")]
use python::Config;

pub const LAUNCHER_TITLE: &str = "Bevy Shell - Template";

pub fn base_app(seed: u64, timestep: Option<f64>) -> App {
    let mut main_system = SystemSet::new()
        .with_system(reset)
        .with_system(check_boundary_collision)
        .with_system(spawn_asteroids)
        .with_system(detect_collisions)
        .with_system(expire_bullets)
        .with_system(cooldowns)
        .with_system(ai);
    if let Some(timestep) = timestep {
        main_system =
            main_system.with_run_criteria(FixedTimestep::step(timestep));
    }
    let mut app = App::new();
    app.add_plugin(PhysicsPlugin::default())
        .insert_resource(SmallRng::seed_from_u64(seed))
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(Score(0))
        .insert_resource(RemainingTime(2700))
        .add_event::<GameOver>()
        .add_system_set(main_system)
        .insert_non_send_resource(Player(agent::random()));
    app
}

pub fn app(agent_path: Option<String>) -> App {
    let mut app = base_app(0, Some(1.0 / 90.0));
    app.insert_resource(WindowDescriptor {
        title: LAUNCHER_TITLE.to_string(),
        width: 2000.0,
        height: 1000.0,
        canvas: Some("#bevy".to_string()),
        fit_canvas_to_parent: true,
        ..Default::default()
    })
    .insert_non_send_resource(match agent_path {
        Some(path) => Player(agent::load(path)),
        None => Player(agent::random()),
    })
    .add_system(keyboard_events)
    .add_plugins(DefaultPlugins)
    .add_startup_system(setup);
    app
}

#[cfg(feature = "python")]
pub fn run_headless(
    _: Config,
    agent: entity_gym_rs::agent::TrainAgent,
    seed: u64,
) {
    use bevy::app::ScheduleRunnerSettings;
    use bevy::asset::AssetPlugin;
    use bevy::render::mesh::MeshPlugin;
    use std::time::Duration;
    base_app(seed, None)
        .insert_resource(ScheduleRunnerSettings::run_loop(
            Duration::from_secs_f64(0.0),
        ))
        .insert_non_send_resource(Player(Box::new(agent)))
        .add_plugins(MinimalPlugins)
        .add_plugin(AssetPlugin::default())
        .add_plugin(MeshPlugin)
        .add_plugin(MaterialPlugin::<StandardMaterial>::default())
        .add_plugin(MaterialPlugin::<ColorMaterial>::default())
        .add_startup_system(setup)
        .run();
}

#[allow(clippy::too_many_arguments)]
fn reset(
    mut game_over: EventReader<GameOver>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<Entity, With<Asteroid>>,
    mut score: ResMut<Score>,
    mut fighter: Query<Entity, With<Fighter>>,
    mut remaining_time: ResMut<RemainingTime>,
) {
    if let Some(GameOver) = game_over.iter().next() {
        println!("Game Over! Score: {}", score.0);
        score.0 = 0;
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
            acceleration: 20.0,
            turn_speed: 0.07,
            bullet_speed: 1500.0,
            bullet_lifetime: 45,
            bullet_cooldown: 13,
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
        //.insert(Velocity::from_linear(Vec3::new(0.0, 50.0, 0.0)))
        .insert(Velocity::from_linear(Vec3::new(0.0, 0.0, 0.0)))
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
    mut score: ResMut<Score>,
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
                    if asteroid.health <= 0.0 {
                        cmd.entity(data2.rigid_body_entity()).despawn();
                        score.0 += 1;
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
                    if asteroid.health <= 0.0 {
                        cmd.entity(data1.rigid_body_entity()).despawn();
                        score.0 += 1;
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
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    keys: Res<Input<KeyCode>>,
    // mut key_evr: EventReader<KeyboardInput>,
    mut fighter: Query<(&mut Fighter, &mut Transform, &mut Velocity)>,
) {
    //     let (mut knife_transform, _) = knife.get_mut(*knife_id).unwrap();
    // *knife_transform = Transform {
    //     translation: Vec3::new(pos.x * 23.0, pos.y * 23.0, 0.0),
    //     rotation: Quat::from_axis_angle(
    //         Vec3::new(0.0, 0.0, 1.0),
    //         pos.y.atan2(pos.x) - f32::consts::PI / 2.0,
    //     ),
    //     //Quat::from_xyzw(pos.x, pos.y, 0.0, 0.0),
    //     ..default()
    // };
    for (mut fighter, mut transform, mut velocity) in &mut fighter {
        let rot = transform.rotation.xyz();
        let mut angle = transform.rotation.to_axis_angle().1;
        if rot.z < 0.0 {
            angle = -angle;
        }
        let angle2 = angle + std::f32::consts::PI / 2.0;

        if keys.pressed(KeyCode::Up) || keys.pressed(KeyCode::W) {
            let thrust = Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                * fighter.acceleration;
            velocity.linear += thrust;
            if velocity.linear.length() > fighter.max_velocity {
                velocity.linear =
                    velocity.linear.normalize() * fighter.max_velocity;
            }
        }
        let rotation =
            if keys.pressed(KeyCode::Left) || keys.pressed(KeyCode::A) {
                Some(fighter.turn_speed)
            } else if keys.pressed(KeyCode::Right) || keys.pressed(KeyCode::D) {
                Some(-fighter.turn_speed)
            } else {
                None
            };
        if let Some(dr) = rotation {
            *transform = Transform {
                rotation: Quat::from_rotation_z(angle + dr),
                ..*transform
            };
        }

        if keys.pressed(KeyCode::Space)
            && fighter.remaining_bullet_cooldown <= 0
        {
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
            fighter.remaining_bullet_cooldown = fighter.bullet_cooldown as i32;
        }

        // if keys.pressed(KeyCode::Left) {
        //     starship. += STARSHIP_ROTATION_SPEED;
        // } else if keys.pressed(KeyCode::Right) {
        //     starship.rotation_angle -= STARSHIP_ROTATION_SPEED;
        // }

        // if keys.pressed(KeyCode::Up) {
        //     velocity.0 += starship.direction() * STARSHIP_ACCELERATION;

        //     if velocity.0.length() > STARSHIP_MAX_VELOCITY {
        //         velocity.0 =
        //             velocity.0.normalize_or_zero() * STARSHIP_MAX_VELOCITY;
        //     }
        // }

        // for evt in key_evr.iter() {
        //     if let (ButtonState::Pressed, Some(KeyCode::Space)) =
        //         (evt.state, evt.key_code)
        //     {
        //         commands
        //             .spawn()
        //             .insert(Bullet {
        //                 start: starship_position.0.clone(),
        //             })
        //             .insert(Position(starship_position.0.clone()))
        //             .insert(Velocity(
        //                 starship.direction().normalize() * BULLET_VELOCITY,
        //             ))
        //             .insert_bundle(MaterialMesh2dBundle {
        //                 mesh: meshes
        //                     .add(Mesh::from(shape::Circle::default()))
        //                     .into(),
        //                 transform: Transform::default()
        //                     .with_scale(Vec3::splat(5.0))
        //                     .with_translation(Vec3::splat(0.0)),
        //                 material: materials.add(ColorMaterial::from(
        //                     Color::rgba(1.0, 1.0, 1.0, 1.0),
        //                 )),
        //                 ..default()
        //             });
        //     }
        // }
    }
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
) {
    for mut fighter in &mut fighter.iter_mut() {
        fighter.remaining_bullet_cooldown -= 1;
    }
    timer.0 -= 1;
    if timer.0 == 0 {
        game_over.send(GameOver);
    }
}

#[allow(clippy::too_many_arguments)]
fn ai(
    mut cmd: Commands,
    mut player: NonSendMut<Player>,
    mut fighter: Query<(&mut Fighter, &mut Transform, &mut Velocity)>,
    mut exit: EventWriter<AppExit>,
    asteroids: Query<(&Asteroid, &Transform, &Velocity), Without<Fighter>>,
    remaining_time: Res<RemainingTime>,
    score: Res<Score>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if let Some((mut fighter, mut transform, mut velocity)) =
        fighter.iter_mut().next()
    {
        let pos = transform.translation;
        let vel = velocity.linear;
        let obs = Obs::new(score.0 as f32)
            // TODO direction
            .entities([FighterFeats {
                x: pos.x,
                y: pos.y,
                dx: vel.x,
                dy: vel.y,
                direction_x: 0.0,
                direction_y: 0.0,
                remaining_time: remaining_time.0,
            }])
            .entities(asteroids.iter().map(
                |(asteroid, transform, velocity)| {
                    let pos = transform.translation;
                    let vel = velocity.linear;
                    AsteroidFeats {
                        health: asteroid.health,
                        radius: asteroid.radius,
                        x: pos.x,
                        y: pos.y,
                        dx: vel.x,
                        dy: vel.y,
                    }
                },
            ));
        let action = player.0.act::<FighterAction>(&obs);
        match action {
            Some(a) => {
                let rot = transform.rotation.xyz();
                let mut angle = transform.rotation.to_axis_angle().1;
                if rot.z < 0.0 {
                    angle = -angle;
                }
                let angle2 = angle + std::f32::consts::PI / 2.0;

                match a {
                    FighterAction::TurnLeft => {
                        *transform = Transform {
                            rotation: Quat::from_rotation_z(
                                angle + fighter.turn_speed,
                            ),
                            ..*transform
                        };
                    }
                    FighterAction::TurnRight => {
                        *transform = Transform {
                            rotation: Quat::from_rotation_z(
                                angle - fighter.turn_speed,
                            ),
                            ..*transform
                        };
                    }
                    FighterAction::Thrust => {
                        let thrust = Vec3::new(angle2.cos(), angle2.sin(), 0.0)
                            * fighter.acceleration;
                        velocity.linear += thrust;
                        if velocity.linear.length() > fighter.max_velocity {
                            velocity.linear = velocity.linear.normalize()
                                * fighter.max_velocity;
                        }
                    }
                    FighterAction::Shoot => {
                        if fighter.remaining_bullet_cooldown <= 0 {
                            spawn_bullet(
                                &mut cmd,
                                &mut meshes,
                                &mut materials,
                                transform.translation,
                                velocity.linear
                                    + Vec3::new(
                                        angle2.cos(),
                                        angle2.sin(),
                                        0.0,
                                    ) * fighter.bullet_speed,
                                fighter.bullet_lifetime,
                            );
                            fighter.remaining_bullet_cooldown =
                                fighter.bullet_cooldown as i32;
                        }
                    }
                }
            }
            None => exit.send(AppExit),
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

struct Score(u32);

struct RemainingTime(u32);

struct Player(pub Box<dyn Agent>);

#[derive(Featurizable)]
struct AsteroidFeats {
    health: f32,
    radius: f32,
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
}

#[derive(Featurizable)]
struct FighterFeats {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    direction_x: f32,
    direction_y: f32,
    remaining_time: u32,
}

#[derive(Action)]
enum FighterAction {
    TurnLeft,
    TurnRight,
    Thrust,
    Shoot,
}
