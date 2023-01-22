# Bevy Starfighter

A simple 2D space shooter game with deep neural network opponents built with [Bevy](https://bevyengine.org/) and [EntityGym](https://github.com/entity-neural-network/entity-gym-rs).

https://user-images.githubusercontent.com/12845088/213926967-753af1be-52d9-4958-8b14-af86c1c3afb7.mp4

## Usage

Play the web version [here](https://cswinter.github.io/bevy-starfighter/).
WASD to move, space to shoot.

Run locally (requries [Rust toolchain](https://rustup.rs/)):

```bash
git clone https://github.com/cswinter/bevy-starfighter.git
cd bevy-starfighter
cargo run --bin native-launcher -- --agent-asset=230111-134322-versus-reldir-1024m --ccd --players=2 --ai-act-interval=12 --human-player
```

Run AI against itself:

```bash
cargo run --bin native-launcher -- --agent-asset=230111-134322-versus-reldir-4096m --ccd --players=2 --ai-act-interval=12
```

Train new AI:

```bash
poetry install
poetry run pip install torch==1.12.0+cu113 -f https://download.pytorch.org/whl/cu113/torch_stable.html
poetry run pip install torch-scatter -f https://data.pyg.org/whl/torch-1.12.0+cu113.html
poetry run maturin develop --features=python --release
poetry run python -u train.py --config=train.ron --checkpoint-dir=out
```

## Technical Details

This sections goes into some of the specifics of how to apply [EntityGym Rust](https://github.com/entity-neural-network/entity-gym-rs) to real-time Bevy games that use [Rapier](https://github.com/dimforge/bevy_rapier) as a physics engine.


### Faster than realtime headless mode

To run the game in headless mode faster than realtime during training in a way that keeps the physics identical, we configure Rapier with `TimestepMode::Fixed` ([src/lib.rs#L136](https://github.com/cswinter/bevy-starfighter/blob/b80ea2e620b4fa119e3d8039ecc6f771ad500ea5/src/lib.rs#L136)):

```rust
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
    app.add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
        .insert_resource(RapierConfiguration {
            gravity: Vect::new(0.0, 0.0),
            timestep_mode,
            ..default()
        })
```

Additionally, we configure the Bevy scheduler to run without any delay during frames ([src/lib.rs#L198](https://github.com/cswinter/bevy-starfighter/blob/b80ea2e620b4fa119e3d8039ecc6f771ad500ea5/src/lib.rs#L198)):

```rust
    app.insert_resource(ScheduleRunnerSettings::run_loop(
        Duration::from_secs_f64(0.0),
    ))
```

### Faster physics during training

To speed up the AI and bring its abilities closer to those of a human player, we only allow it to take an action every `ai_act_interval` frames (by default, every 12 frames = 133ms).
While training, we don't really care about the intermediate physics steps, so we can speed up the simulation by not calculating intermediary frames.
For some reason, the fidelity of the simulation degrades when skipping too many frames.
Empirically, combining up to 4 physics steps into a single frame (`--frameskip=4`) still gives fairly accurate physics.

Command to observe game with 4x accelerated physics:

```bash
cargo run --bin native-launcher -- --agent-asset=230111-134322-versus-reldir-4096m --ccd --players=2 --ai-act-interval=12 --frameskip=4
```
