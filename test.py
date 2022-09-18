import hyperstate
from enn_trainer import TrainConfig, State, init_train_state, train, EnvConfig
from entity_gym.env import VecEnv
from entity_gym_rs import RustVecEnv
from ragged_buffer import RaggedBufferI64
import numpy as np

from bevy_dogfight_ai import create_env, Config


def create_dogfight_vec_env(
    cfg: EnvConfig, num_envs: int, num_processes: int, first_env_index: int
) -> VecEnv:
    env = create_env(
        Config(),
        num_envs,
        num_processes,
        first_env_index=first_env_index,
    )
    return RustVecEnv(env)  # type: ignore

turn_left = RaggedBufferI64.from_flattened(np.array([[1]], dtype=np.int64), lengths=np.array([1], dtype=np.int64))
thrust = RaggedBufferI64.from_flattened(np.array([[2]], dtype=np.int64), lengths=np.array([1], dtype=np.int64))
shoot = RaggedBufferI64.from_flattened(np.array([[3]], dtype=np.int64), lengths=np.array([1], dtype=np.int64))

env = create_dogfight_vec_env(EnvConfig(), 1, 1, 0)
print(env.reset(None))
for action in [turn_left, thrust, shoot, turn_left, thrust, thrust]:
    print(env.act({"FighterAction": action}, None))