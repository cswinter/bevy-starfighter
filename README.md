# Bevy Dogfight AI

```bash
# Run game
cargo run -- --act-interval=12
# Run game with trained policy
cargo run -- --act-interval=12 --agent-path=assets/policies/ai12-2m
# Run random AI
cargo run -- --random-ai --act-interval=12
# Run random AI in headless mode
cargo run -- --random-ai --headless --act-interval=12
# More efficient headless mode that does a single physics step
# Misses many bullet collisions
cargo run -- --random-ai --headless --frameskip=12
# Decently accurate accelerated physics
cargo run -- --frameskip=4 --act-interval=12 --agent-path=assets/policies/ai12-8m
cargo run -- --headless --frameskip=4 --act-interval=12 --agent-path=assets/policies/ai12-8m
```