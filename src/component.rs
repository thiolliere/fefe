#[doc(hidden)]
pub use animation::AnimationState;

use ncollide2d::shape::{ConvexPolygon, ShapeHandle};
use nphysics2d::math::Force;
use nphysics2d::object::BodyStatus;
use retained_storage::RetainedStorage;
use specs::{Component, Entity, NullStorage, VecStorage, WriteStorage};
use std::f32::consts::PI;
use itertools::Itertools;

#[derive(Deserialize, Clone, Default, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct Player;

#[derive(Deserialize, Clone, Component)]
#[storage(VecStorage)]
pub struct SwordRifle {
    #[serde(skip, default = "::util::true_bool")]
    pub sword_mode: bool,
    #[serde(skip)]
    pub attack: bool,

    pub sword_damage: usize,
    pub sword_reload_time: f32,
    #[serde(skip)]
    pub sword_reloading: f32,
    pub sword_length: f32,
    pub sword_range: f32,
    #[serde(skip, default = "::util::default_shape_handle")]
    pub sword_shape: ShapeHandle<f32>,

    pub rifle_damage: usize,
    pub rifle_reload_time: f32,
    #[serde(skip)]
    pub rifle_reloading: f32,
}
impl SwordRifle {
    pub fn compute_shapes(&mut self) {
        let div = (16.0 * (self.sword_range / (2.0 * PI))).ceil() as usize;
        let shape = ConvexPolygon::try_new(
            (0..=div)
                .map(|i| -self.sword_range / 2.0 + (i as f32 / div as f32) * self.sword_range)
                .map(|angle| ::na::Point2::new(angle.cos(), angle.sin()))
                .chain(Some(::na::Point2::new(0.0, 0.0)))
                .map(|point| self.sword_length * point)
                .collect::<Vec<_>>(),
        ).unwrap();
        self.sword_shape = ShapeHandle::new(shape);
    }
}

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct Aim(pub f32);

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct Activators(pub Vec<Activator>);

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Activator {
    pub tempo: usize,
    pub partition: Vec<bool>,
    #[serde(skip)]
    pub activated: bool,
    pub sound: ::audio::Sound,
}

//////////////////////////////// Life ////////////////////////////////

/// Only against players
#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct ContactDamage(pub usize);

#[derive(Deserialize, Clone, Default, Component)]
#[serde(deny_unknown_fields)]
#[storage(NullStorage)]
pub struct DeadOnContact;

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct Life(pub usize);

//////////////////////////////// Position ////////////////////////////////

// TODO: maybe add an activator for changing sens
#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct PositionInPath {
    pub velocity: f32,
    #[serde(skip)]
    pub current_point: usize,
    #[serde(skip)]
    pub current_advancement: f32,
    #[serde(skip)]
    pub points: Vec<::na::Vector2<f32>>,
    #[serde(skip)]
    pub distances: Vec<f32>,
}

impl PositionInPath {
    pub fn set(&mut self, path: Vec<::na::Vector2<f32>>) {
        self.distances = path.iter().chain(&[path[0]]).tuple_windows()
            .map(|(p1, p2)| (p2 -p1).norm())
            .collect();
        self.points = path;
    }
}

//////////////////////////////// Velocity ////////////////////////////////

#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct VelocityControl {
    pub velocity: f32,
    #[serde(skip)]
    #[serde(default = "::util::vector_zero")]
    pub direction: ::na::Vector2<f32>,
}

pub const VELOCITY_TO_PLAYER_DISTANCE_TO_GOAL: f32 = 0.1;

/// Go to the closest or the last position in memory
#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct VelocityToPlayerMemory {
    pub activator: usize,
    #[serde(skip)]
    pub last_closest_in_sight: Option<::na::Vector2<f32>>,
    pub velocity: f32,
    /// If false it is equivalent to go to player in sight
    pub memory: bool,
}

/// Go into random directions
/// or closest player in sight depending of proba
///
#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct VelocityToPlayerRandom {
    pub activator: usize,
    /// If some then a random direction with f32 norm is added
    pub random_weighted: Option<f32>,
    /// Clamp the proba with distance to characters
    pub dist_proba_clamp: ::util::ClampFunction,
    /// Clamp the proba with aim of the characters
    pub aim_proba_clamp: ::util::ClampFunction,
    pub velocity: f32,
    pub toward_player: bool,
    #[serde(skip)]
    #[serde(default = "::util::vector_zero")]
    pub current_direction: ::na::Vector2<f32>,
}

#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct VelocityToPlayerCircle {
    pub activator: usize,
    pub circle_velocity: f32,
    pub direct_velocity: f32,
    /// Normal distribution
    pub shift_time: (f64, f64),
    #[serde(skip)]
    pub next_shift: f32,
    #[serde(default)]
    pub dir_shift: bool,
}

// TODO: peut être faire qu'il se repoussent un peu
// peut être aussi faire que pas mis a jour tout le temps
// c'est peut être plus compliqué de faire un truc bien
// mais si on les fait apparaitre dans un cadre autour du héros
// et on les tue si il sorte du cadre ca peut faire un truc
// bien dans une plaine
#[derive(Component)]
#[storage(VecStorage)]
pub struct Boid {
    pub id: usize,
    pub clamp: ::util::ClampFunction,
    pub velocity: f32,
    pub weight: f32,
}

/// The processor takes distance with player aim in radiant
/// The velocity is multiplied by the result
#[derive(Deserialize, Clone, Deref, Component)]
#[storage(VecStorage)]
#[serde(deny_unknown_fields)]
pub struct VelocityDistanceDamping(pub ::util::ClampFunction);

/// The processor takes distance with player
/// The velocity is multiplied by the result
#[derive(Deserialize, Clone, Deref, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct VelocityAimDamping(pub ::util::ClampFunction);

//////////////////////////////// Force ////////////////////////////////

/// The processor takes distance with player aim in radiant
/// The final damping is divided by the result
#[derive(Deserialize, Clone, Deref, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct PlayersAimDamping(pub ::util::ClampFunction);

/// The processor takes distance with player
/// The final damping is divided by the result
#[derive(Deserialize, Clone, Deref, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct PlayersDistanceDamping(pub ::util::ClampFunction);

#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct GravityToPlayers {
    pub force: f32,
    pub powi: i32,
}

#[derive(Component)]
#[storage(VecStorage)]
pub struct ControlForce(pub Force<f32>);

#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct Damping {
    pub linear: f32,
    pub angular: f32,
}

//////////////////////////////// Spawner ////////////////////////////////
//TODO: shoot to hero spawner

/// Spawn an entity if character is in aim at a certain probability function of
/// the distance to the character every time activated
#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct UniqueSpawner {
    pub activator: usize,
    pub spawn: String,
    /// Clamp the proba with distance to characters
    pub dist_proba_clamp: Option<::util::ClampFunction>,
    /// Clamp the proba with aim of the characters
    pub aim_proba_clamp: Option<::util::ClampFunction>,
}

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct TurretSpawner(pub Vec<TurretPart>);

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TurretPart {
    pub activator: usize,
    pub rotation_activator: Option<usize>,
    pub spawn: String,
    pub rotation_time: usize,
    pub clockwise: bool,
    // TODO: maybe use a vec here so it can have multiple canon
    //       or maybe not because can be done with multiple part
    //       but its way more handy that multiple parts
    pub start_time: isize,
    pub shoot_distance: f32,
}

#[derive(Deserialize, Clone, Component)]
#[serde(deny_unknown_fields)]
#[storage(VecStorage)]
pub struct ChamanSpawner {
    pub activator: usize,
    pub spawn: String,
    pub number_of_spawn: usize,
    #[serde(skip)]
    pub spawned: Vec<Entity>,
}

//////////////////////////////// Physic ////////////////////////////////

#[derive(Clone)]
pub struct RigidBody(pub ::nphysics2d::object::BodyHandle);
impl Component for RigidBody {
    type Storage = RetainedStorage<Self, VecStorage<Self>>;
}

#[allow(unused)]
impl RigidBody {
    pub fn safe_insert<'a>(
        entity: Entity,
        position: ::nphysics2d::math::Isometry<f32>,
        local_inertia: ::nphysics2d::math::Inertia<f32>,
        local_center_of_mass: ::nphysics2d::math::Point<f32>,
        status: BodyStatus,
        bodies_handle: &mut WriteStorage<'a, ::component::RigidBody>,
        physic_world: &mut ::resource::PhysicWorld,
        bodies_map: &mut ::resource::BodiesMap,
    ) -> Self {
        let body_handle =
            physic_world.add_rigid_body(position, local_inertia, local_center_of_mass);
        {
            let mut rigid_body = physic_world.rigid_body_mut(body_handle).unwrap();
            rigid_body.set_status(status);
            rigid_body
                .activation_status_mut()
                .set_deactivation_threshold(None);
        }
        bodies_map.insert(body_handle, entity);

        bodies_handle.insert(entity, RigidBody(body_handle));
        RigidBody(body_handle)
    }

    #[inline]
    #[allow(unused)]
    pub fn get<'a>(
        &'a self,
        physic_world: &'a ::resource::PhysicWorld,
    ) -> &'a ::nphysics2d::object::RigidBody<f32> {
        physic_world
            .rigid_body(self.0)
            .expect("Rigid body in specs does not exist in physic world")
    }

    #[inline]
    pub fn get_mut<'a>(
        &self,
        physic_world: &'a mut ::resource::PhysicWorld,
    ) -> &'a mut ::nphysics2d::object::RigidBody<f32> {
        physic_world
            .rigid_body_mut(self.0)
            .expect("Rigid body in specs does not exist in physic world")
    }
}

#[derive(Default, Component)]
#[storage(NullStorage)]
pub struct Ground;

#[derive(Deref, DerefMut, Component)]
#[storage(VecStorage)]
pub struct Contactor(pub Vec<Entity>);

//////////////////////////////// Debug ////////////////////////////////

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[storage(VecStorage)]
#[serde(deny_unknown_fields)]
pub struct DebugCircles(pub Vec<f32>);

#[derive(Deserialize, Clone, Deref, DerefMut, Component)]
#[storage(VecStorage)]
#[serde(deny_unknown_fields)]
pub struct DebugRays(pub Vec<f32>);

#[derive(Deserialize, Clone, Component)]
#[storage(VecStorage)]
#[serde(deny_unknown_fields)]
pub struct DebugColor(pub usize);

//////////////////////////////// TODO ////////////////////////////////

// TODO: for bullet
//       position is function of t along an axis ?
// pub struct Positionned {}

// TODO: do something gravity like ! with inertia
// // Decrease of sound: -6dB
// // The sound pressure level (SPL) decreases with doubling of distance by (−)6 dB.
// /// This component store the position of the last heard sound
// /// compute the main position
// /// heard sound intensity decrease over time
// pub struct EarPositionMemory {
//     heards: Vec<(::na::Vector2<f32>, f32)>,
//     position: ::na::Vector2<f32>,
//     db: f32,
// }
// // TODO: this can be done better with just position and db and updated each time by decreasing the
// // memoy and add new heards
// // MAYBE: impl some gravity like for sound:
// // sound create a mass at a point during a frame
// // memory is only consequence of intertia

// impl EarPositionMemory {
//     pub fn add_heard(&mut self, heard_position: ::na::Vector2<f32>, heard_db: f32) {
//         self.heards.push((heard_position, heard_db));
//         self.recompute();
//     }

//     pub fn recompute(&mut self) {
//         let (position_sum, db_sum) = self.heards.iter()
//             // FIXME: This mean may have to be on sound pressure instead of dB
//             .fold((::na::zero(), 0.0), |acc: (::na::Vector2<f32>, f32), &(position, db)| (acc.0+position*db, acc.1+db));
//         self.position = position_sum/db_sum;
//         self.db = db_sum;
//     }
// }

// impl Component for EarPositionMemory {
//     type Storage = VecStorage<Self>;
// }

// // TODO: or better have a resource with a channel for send and one for receive
// pub fn play_sound(position: ::na::Vector2<f32>, db: f32, world: &mut World) {
//     for (listener, body) in (
//         &mut world.write::<::component::EarPositionMemory>(),
//         &world.read::<::component::RigidBody>()
//     ).join() {
//         // TODO: computation with a resource for gas constant ?
//         // let listener_position = body.get(&world.read_resource()).position().translation.vector;
//         // let distance = (position - listener_position).norm();
//         // if db*(-distance).exp() > listener.hear_limit {
//         //     listener.hear_position = Some(position);
//         // }
//     }
// }
