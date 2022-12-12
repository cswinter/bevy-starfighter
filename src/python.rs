use crate::*;

use entity_gym_rs::agent::TrainEnvBuilder;
use entity_gym_rs::low_level::py_vec_env::PyVecEnv;
use pyo3::prelude::*;

#[derive(Clone)]
#[pyclass]
pub struct Config {
    pub frameskip: u32,
    pub act_interval: u32,
    pub versus: bool,
    pub ccd: bool,
}

#[pymethods]
impl Config {
    #[new]
    #[args(frameskip = "1", act_interval = "1", versus = "true", ccd = "true")]
    fn new(frameskip: u32, act_interval: u32, versus: bool, ccd: bool) -> Self {
        Config {
            frameskip,
            act_interval,
            versus,
            ccd,
        }
    }
}

#[pyfunction]
fn create_env(
    config: Config,
    num_envs: usize,
    threads: usize,
    first_env_index: u64,
) -> PyVecEnv {
    let builder = TrainEnvBuilder::default()
        .entity::<entity::Fighter>()
        .entity::<entity::EnemyFighter>()
        .entity::<entity::Asteroid>()
        .entity::<entity::Bullet>()
        .action::<act::FighterAction>();
    if config.versus {
        builder.build_multiagent::<_, _, 2>(
            config,
            super::train2,
            num_envs,
            threads,
            first_env_index,
        )
    } else {
        builder.build(config, super::train1, num_envs, threads, first_env_index)
    }
}

#[pymodule]
fn bevy_starfighter(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_env, m)?)?;
    m.add_class::<Config>()?;
    Ok(())
}
