#[cfg(feature = "python")]
fn main() {
    use bevy_dogfight_ai::python::Config;
    use bevy_dogfight_ai::*;
    use entity_gym_rs::agent::TrainEnvBuilder;
    use ragged_buffer::ragged_buffer::RaggedBuffer;
    //use std::hint::black_box;
    use clap::Parser;
    use std::time::Instant;

    #[derive(Parser, Debug)]
    #[clap(author, version, about, long_about = None)]
    struct Args {
        #[clap(long, value_parser, default_value = "10")]
        steps: usize,
        #[clap(long, value_parser, default_value = "1")]
        frameskip: u32,
        #[clap(long, value_parser, default_value = "12")]
        act_interval: u32,
        #[clap(long, value_parser, default_value = "128")]
        environments: usize,
        #[clap(long, value_parser, default_value = "4")]
        threads: usize,
    }
    let args = Args::parse();

    let start_time = Instant::now();
    let config = Config {
        frameskip: args.frameskip,
        act_interval: args.act_interval,
        versus: false,
    };
    let mut env = TrainEnvBuilder::default()
        .entity::<entity::Fighter>()
        .entity::<entity::EnemyFighter>()
        .entity::<entity::Asteroid>()
        .entity::<entity::Bullet>()
        .action::<act::FighterAction>()
        .build(config, train1, args.environments, args.threads, 0)
        .env;
    let steps = args.steps;
    env.reset();
    for i in 0..steps {
        let _obs = env.act(vec![Some(RaggedBuffer::<i64> {
            data: (0..128).map(|j| j * i as i64 * 991 % 4).collect(),
            subarrays: (0..128).map(|i| i..i + 1).collect(),
            features: 1,
            items: 128,
        })]);
        //black_box(obs);
    }
    let throughput = steps as f64 * env.num_envs as f64
        / (start_time.elapsed().as_secs() as f64)
        / 1000.0;
    println!("{} K samples/s", throughput);
}

#[cfg(not(feature = "python"))]
fn main() {
    println!("Compile with --features=python");
}
