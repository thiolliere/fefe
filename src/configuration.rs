use std::fs::File;
use std::path::PathBuf;

const FILENAME: &str = "configuration.ron";

lazy_static! {
    pub static ref CFG: Configuration = {
        let file = File::open(FILENAME).unwrap();
        let c: Configuration = ::ron::de::from_reader(file).unwrap();
        c.check();
        c
    };
}

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    pub animation: ::animation::AnimationsCfg,
    pub fps: usize,
    pub physic_max_timestep: f32,
    pub physic_min_timestep: f32,
    pub map_directory: PathBuf,
}

impl Configuration {
    fn check(&self) {
        assert!(self.map_directory.is_dir());
    }
}
