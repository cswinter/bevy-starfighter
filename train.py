import json
import hyperstate
from enn_trainer import TrainConfig, State, init_train_state, train, EnvConfig
from entity_gym.env import VecEnv
from entity_gym_rs import RustVecEnv

from bevy_starfighter import create_env, Config


def create_starfighter_vec_env(
    cfg: EnvConfig, num_envs: int, num_processes: int, first_env_index: int
) -> VecEnv:
    kwargs = json.loads(cfg.kwargs)
    env = create_env(
        Config(**kwargs),
        num_envs,
        num_processes,
        first_env_index=first_env_index,
    )
    return RustVecEnv(env)  # type: ignore


@hyperstate.stateful_command(TrainConfig, State, init_train_state)
def main(state_manager: hyperstate.StateManager) -> None:
    train(state_manager=state_manager, env=create_starfighter_vec_env)


if __name__ == "__main__":
    main()
