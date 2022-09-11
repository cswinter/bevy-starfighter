use bevy::prelude::shape::{Circle, Quad};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::sprite::MaterialMesh2dBundle;
use heron::prelude::*;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub const LAUNCHER_TITLE: &str = "Bevy Shell - Template";

pub fn app() -> App {
    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: LAUNCHER_TITLE.to_string(),
        canvas: Some("#bevy".to_string()),
        fit_canvas_to_parent: true,
        ..Default::default()
    })
    .add_plugin(PhysicsPlugin::default())
    .insert_resource(SmallRng::seed_from_u64(42))
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .add_event::<GameOver>()
    .add_system(keyboard_events)
    .add_system(reset)
    .add_system(check_boundary_collision)
    .add_system(spawn_asteroids)
    .add_system(detect_collisions)
    .add_plugins(DefaultPlugins)
    .add_startup_system(setup);
    app
}

fn reset(
    mut game_over: EventReader<GameOver>,
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<Entity, With<Asteroid>>,
) {
    if let Some(GameOver) = game_over.iter().next() {
        // Despawn all entities
        for entity in query.iter_mut() {
            cmd.entity(entity).despawn_recursive();
        }
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

fn detect_collisions(
    mut cmd: Commands,
    mut events: EventReader<CollisionEvent>,
    collision_type: Query<&CollisionType>,
    mut game_over: EventWriter<GameOver>,
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
            .insert(Asteroid)
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
    // mut commands: Commands,
    // mut meshes: ResMut<Assets<Mesh>>,
    // mut materials: ResMut<Assets<ColorMaterial>>,
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
    for (fighter, mut transform, mut velocity) in &mut fighter {
        let rotation =
            if keys.pressed(KeyCode::Left) || keys.pressed(KeyCode::A) {
                Some(fighter.turn_speed)
            } else if keys.pressed(KeyCode::Right) || keys.pressed(KeyCode::D) {
                Some(-fighter.turn_speed)
            } else {
                None
            };
        if let Some(dr) = rotation {
            let rot = transform.rotation.xyz();
            let mut angle = transform.rotation.to_axis_angle().1;
            if rot.z < 0.0 {
                angle = -angle;
            }
            *transform = Transform {
                rotation: Quat::from_rotation_z(angle + dr),
                ..*transform
            };
        }

        if keys.pressed(KeyCode::Up) || keys.pressed(KeyCode::W) {
            let rot = transform.rotation.xyz();
            let mut angle = transform.rotation.to_axis_angle().1;
            if rot.z < 0.0 {
                angle = -angle;
            }
            angle += std::f32::consts::PI / 2.0;
            let thrust =
                Vec3::new(angle.cos(), angle.sin(), 0.0) * fighter.acceleration;
            velocity.linear += thrust;
            if velocity.linear.length() > fighter.max_velocity {
                velocity.linear =
                    velocity.linear.normalize() * fighter.max_velocity;
            }
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

#[derive(Component)]
struct Fighter {
    max_velocity: f32,
    acceleration: f32,
    turn_speed: f32,
}

#[derive(Component)]
struct Asteroid;

#[derive(Component)]
enum CollisionType {
    Asteroid,
    Fighter,
}

struct GameOver;
