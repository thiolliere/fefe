use animation::{AnimationName, AnimationSpecie};
use entity::{InsertPosition, Insertable};
use ncollide2d::shape::{Ball, ShapeHandle};
use nphysics2d::math::Force;
use nphysics2d::object::{BodyStatus, Material};
use nphysics2d::volumetric::Volumetric;
use specs::World;

#[derive(Serialize, Deserialize)]
pub struct Player;

impl Insertable for Player {
    fn insert(&self, position: InsertPosition, world: &mut World) {
        let entity = world
            .create_entity()
            .with(::component::AnimationState::new(
                AnimationSpecie::Character,
                AnimationName::IdleRifle,
            ))
            .with(::component::Player)
            .with(::component::Aim(position.rotation.angle()))
            .with(::component::Life(1))
            .with(::component::ControlForce(Force::zero()))
            .with(::component::Damping {
                linear: ::CFG.player_linear_damping,
                angular: ::CFG.player_angular_damping,
            })
            .build();

        let mut physic_world = world.write_resource::<::resource::PhysicWorld>();

        let shape = ShapeHandle::new(Ball::new(::CFG.player_radius));
        let body_handle = ::component::RigidBody::safe_insert(
            entity,
            position.0,
            shape.inertia(1.0),
            shape.center_of_mass(),
            BodyStatus::Dynamic,
            &mut world.write(),
            &mut physic_world,
            &mut world.write_resource(),
        );

        physic_world.add_collider(
            0.0,
            shape,
            body_handle,
            ::na::one(),
            Material::new(0.0, 0.0),
        );
    }
}
