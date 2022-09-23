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
cargo run -- --random-ai --headless --frameskip=12
```