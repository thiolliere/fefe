use specs::{Component, VecStorage};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::path::PathBuf;

#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub enum Framerate {
    /// Distance for one loop
    Walk(f32),
    /// Image per second
    Fix(f32),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationsConf {
    pub table: HashMap<(AnimationSpecie, AnimationName), Vec<String>>,
    pub parts: HashMap<String, AnimationPartConf>,
    pub directory: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationPartConf {
    pub filename: String,
    pub layer: f32,
    pub framerate: Framerate,
}

lazy_static! {
    pub(crate) static ref ANIMATIONS: Animations = Animations::load().unwrap();
}

/// Animation parts must not be empty
pub(crate) struct Animations {
    pub images: Vec<PathBuf>,
    table: HashMap<(AnimationSpecie, AnimationName), CompleteAnimation>,
}

impl Animations {
    fn load() -> Result<Animations, ::failure::Error> {
        let animations_cfg: AnimationsConf =
            ::ron::de::from_reader(File::open("data/animation.ron")?)?;

        let mut parts_table = HashMap::new();
        let mut images = vec![];

        let mut dir_entries = vec![];
        for entry in fs::read_dir(&animations_cfg.directory).map_err(|e| {
            format_err!(
                "read dir \"{}\": {}",
                animations_cfg.directory.to_string_lossy(),
                e
            )
        })? {
            let entry = entry
                .map_err(|e| {
                    format_err!(
                        "read dir \"{}\": {}",
                        animations_cfg.directory.to_string_lossy(),
                        e
                    )
                })?
                .path();

            if entry.extension().iter().any(|p| *p == OsStr::new("png")) {
                dir_entries.push(entry);
            }
        }

        for (part_name, part) in &animations_cfg.parts {
            let mut part_images = dir_entries
                .iter()
                .filter(|p| {
                    if let Some(stem) = p.file_stem() {
                        let len = stem.len();
                        let stem_string = stem.to_string_lossy();
                        let (name, _number) = stem_string.split_at(len - 4);
                        name == part_name
                    } else {
                        false
                    }
                })
                .cloned()
                .collect::<Vec<_>>();

            if part_images.len() == 0 {
                return Err(format_err!(
                    "invalid animation configuration: \"{}\" have no images in \"{}\"",
                    part_name,
                    animations_cfg.directory.to_string_lossy()
                ));
            }

            part_images.sort();

            parts_table.insert(
                part_name,
                AnimationPart {
                    framerate: part.framerate,
                    layer: part.layer,
                    images: part_images
                        .iter()
                        .enumerate()
                        .map(|(i, _)| i + images.len())
                        .collect(),
                },
            );

            images.append(&mut part_images);
        }

        let mut table = HashMap::new();

        for (&key, part_names) in &animations_cfg.table {
            let mut parts = vec![];
            for part_name in part_names {
                let part = parts_table.get(&part_name)
                    .ok_or(format_err!("invalid animation configuration: \"{}\" does not correspond to any animation part", part_name))?;
                parts.push(part.clone());
            }
            let complete_animation = CompleteAnimation::new(parts).map_err(|e| {
                format_err!("invalid animation configuration: \"{:?}\": {} ", key, e)
            })?;
            table.insert(key, complete_animation);
        }

        Ok(Animations { images, table })
    }
}

#[derive(Deserialize, Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub enum AnimationName {
    Idle,
    Walk,
    SwordAttack,
}

#[derive(Deserialize, Hash, PartialEq, Eq, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub enum AnimationSpecie {
    Character,
    Bomb,
}

#[derive(Clone)]
#[doc(hidden)]
/// images must not be empty
pub struct AnimationPart {
    pub layer: f32,
    framerate: Framerate,
    images: Vec<usize>,
}

#[derive(Clone)]
#[doc(hidden)]
pub struct CompleteAnimation {
    pub duration: f32,
    pub parts: Vec<AnimationPart>,
}

impl CompleteAnimation {
    fn new(parts: Vec<AnimationPart>) -> Result<Self, ::failure::Error> {
        let duration = parts
            .iter()
            .filter_map(|a| a.duration())
            .max_by(|i, j| i.partial_cmp(j).unwrap())
            .ok_or(format_err!("Animation contains no sized parts"))?;

        Ok(CompleteAnimation { parts, duration })
    }
}

impl AnimationPart {
    pub fn image_at(&self, timer: f32, distance: f32) -> usize {
        let len = self.images.len();
        match self.framerate {
            Framerate::Walk(r) => {
                let i = ((distance / r) * len as f32).floor() as usize;
                self.images[i % len]
            }
            Framerate::Fix(r) => {
                let i = (timer * r).floor() as usize;
                self.images[i % len]
            }
        }
    }

    fn duration(&self) -> Option<f32> {
        match self.framerate {
            Framerate::Walk(_) => None,
            Framerate::Fix(r) => Some(self.images.len() as f32 * r),
        }
    }
}

#[doc(hidden)]
pub struct AnimationState {
    /// 0 is no walk
    pub distance: f32,
    pub specie: AnimationSpecie,
    pub idle_animation: CompleteAnimation,
    pub animations: Vec<CompleteAnimation>,
    pub timer: f32,
}

impl AnimationState {
    pub fn new(specie: AnimationSpecie, idle_animation: AnimationName) -> Self {
        AnimationState {
            distance: 0.0,
            specie,
            idle_animation: ANIMATIONS.table[&(specie, idle_animation)].clone(),
            animations: vec![],
            timer: 0.0,
        }
    }
}

impl Component for AnimationState {
    type Storage = VecStorage<Self>;
}

#[derive(Deref, DerefMut)]
#[doc(hidden)]
pub struct AnimationImages(pub Vec<AnimationImage>);

#[doc(hidden)]
pub struct AnimationImage {
    pub id: usize,
    pub position: ::na::Isometry2<f32>,
    pub layer: f32,
}
