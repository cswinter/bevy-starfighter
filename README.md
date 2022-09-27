# Bevy Dogfight AI

```bash
# Run game
cargo run --bin native-launcher  -- --act-interval=12

# Run game with trained policy
cargo run --bin native-launcher  -- --act-interval=12 --agent-path=assets/policies/ma-ai12fs4-64m

# Run random AI
cargo run --bin native-launcher  -- --random-ai --act-interval=12

# Run random AI in headless mode
cargo run --bin native-launcher  -- --random-ai --headless --act-interval=12

# More efficient headless mode that does a single physics step
# Misses many bullet collisions
cargo run --bin native-launcher -- --random-ai --headless --frameskip=12

# Decently accurate accelerated physics
cargo run --bin native-launcher -- --frameskip=4 --act-interval=12 --agent-path=assets/policies/ma-ai12fs4-64m
cargo run --bin native-launcher -- --headless --frameskip=4 --act-interval=12 --agent-path=assets/policies/ma-ai12fs4-64m

# Continuous collision detection seems to help, but also causes weird issue where bullets are "stuck" to an asteroid without causing collision events
# Observe:
cargo run --bin native-launcher -- --frameskip=4 --act-interval=12 --agent-path=assets/policies/ma-ai12fs4-64m --ccd
# Also still getting tunelling with frameskip=12
cargo run --bin native-launcher -- --frameskip=12 --act-interval=12 --agent-path=assets/policies/ma-ai12fs4-64m --ccd
```