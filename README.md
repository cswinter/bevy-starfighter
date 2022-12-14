# Bevy Starfighter

A simple 2D space shooter game with deep neural network opponents built with [Bevy](https://bevyengine.org/) and [EntityGym](https://github.com/entity-neural-network/entity-gym-rs).

https://user-images.githubusercontent.com/12845088/194914852-359bdcc7-92b0-4b11-8797-c50b1b767f67.mp4


## Usage

Play the web version [here](https://cswinter.github.io/bevy-starfighter/).
WAD to move, space to shoot.

Run locally:

```bash
git clone https://github.com/cswinter/bevy-starfighter.git
cd bevy-starfighter
cargo run --bin native-launcher -- --agent-asset=versus-relpos-obsfix-128m --ccd --players=2 --ai-act-interval=12 --human-player
```

Run AI against itself:

```bash
cargo run --bin native-launcher -- --agent-asset=versus-relpos-obsfix-512m --ccd --players=2 --ai-act-interval=12
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

This sections goes into some of the specifics of how to apply [EntityGym Rust](https://github.com/entity-neural-network/entity-gym-rs) to real-time Bevy games that use [Heron](https://github.com/jcornaz/heron) as a physics engine.


### Basic setup

To run the game in headless mode faster than realtime during training in a way that keeps the physics identical, we configure Heron to use a fixed `PhysicsSteps`:

```rust
app.insert_resource(
    PhysicsSteps::every_frame(
        Duration::from_secs_f64(settings.timestep_secs() as f64)
    )
);
```

During deployment, we run the Bevy App with the same `FixedTimestep` to match real-time, while during training we run the the App without any delay during frames:

```rust
// During deployment
main_system.with_run_criteria(
    FixedTimestep::step(settings.timestep_secs() as f64)
);

// During training
app.insert_resource(
    ScheduleRunnerSettings::run_loop(
        Duration::from_secs_f64(0.0)
    )
);
```

### Faster physics during training

To speed up the AI and bring its abilities closer to those of a human player, we only allow it to take an action every `ai_act_interval` frames (by default, every 12 frames = 133ms).
While training, we don't really care about the intermediate physics steps, so we can speed up the simulation by reducing the number of frames and increasing the physics timestep.
For some reason, the fidelity of the simulation degrades when skipping too many frames.
Empirically, combining up to 4 physics steps into a single frame (`--frameskip=4`) still gives fairly accurate physics.

Observing game with 4x accelerated physics:

```bash
cargo run --bin native-launcher -- --frameskip=4 --ai-act-interval=12 --agent-asset=versus-relpos-obsfix-512m --ccd
```