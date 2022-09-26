use crate::*;

use entity_gym_rs::agent::TrainEnvBuilder;
use entity_gym_rs::low_level::py_vec_env::PyVecEnv;
use pyo3::prelude::*;

#[derive(Clone)]
#[pyclass]
pub struct Config {
    pub frameskip: u32,
    pub act_interval: u32,
}

#[pymethods]
impl Config {
    #[new]
    #[args(frameskip = "1", act_interval = "1")]
    fn new(frameskip: u32, act_interval: u32) -> Self {
        Config { frameskip, act_interval }
    }
}

#[pyfunction]
fn create_env(
    config: Config,
    num_envs: usize,
    threads: usize,
    first_env_index: u64,
) -> PyVecEnv {
    TrainEnvBuilder::default()
        .entity::<entity::Fighter>()
        .entity::<entity::Asteroid>()
        .entity::<entity::Bullet>()
        .action::<act::FighterAction>()
        .build(
            config,
            super::run_training,
            num_envs,
            threads,
            first_env_index,
        )
}

#[pymodule]
fn bevy_dogfight_ai(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_env, m)?)?;
    m.add_class::<Config>()?;
    Ok(())
}
