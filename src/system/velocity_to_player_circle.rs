use nphysics2d::math::Velocity;
use specs::{Join, ReadStorage, System, WriteExpect, WriteStorage, ReadExpect};
use std::f32::EPSILON;

pub struct VelocityToPlayerCircleSystem;

impl<'a> System<'a> for VelocityToPlayerCircleSystem {
    type SystemData = (
        ReadStorage<'a, ::component::Player>,
        ReadStorage<'a, ::component::RigidBody>,
        ReadStorage<'a, ::component::Activators>,
        ReadStorage<'a, ::component::Contactor>,
        WriteStorage<'a, ::component::VelocityToPlayerCircle>,
        ReadExpect<'a, ::resource::Audio>,
        WriteExpect<'a, ::resource::PhysicWorld>,
    );

fn run(&mut self, (players, rigid_bodies, activatorses, contactors, mut circle_to_players, audio, mut physic_world): Self::SystemData){
        let players_position = (&players, &rigid_bodies)
            .join()
            .map(|(_, body)| body.get(&physic_world).position().translation.vector)
            .collect::<Vec<_>>();

        for (circle_to_player, rigid_body, contactor, activators) in (
            &mut circle_to_players,
            &rigid_bodies,
            &contactors,
            &activatorses,
        ).join()
        {
            let position = rigid_body.get(&physic_world).position().translation.vector;
            let ref activator = activators[circle_to_player.activator];
            if !contactor.0.is_empty() || activator.activated {
                audio.play(activator.sound, position.into());
                circle_to_player.dir_shift = !circle_to_player.dir_shift;
            }

            let direction = players_position
                .iter()
                .map(|p| (p - position))
                .min_by_key(|p| (p.norm() * ::CMP_PRECISION) as isize)
                .and_then(|p| p.try_normalize(EPSILON));

            if let Some(direction) = direction {
                let orthogonal = ::na::Vector2::new(direction[1], -direction[0]);
                let direct_velocity = direction * circle_to_player.direct_velocity;
                let mut circle_velocity = orthogonal * circle_to_player.direct_velocity;
                if circle_to_player.dir_shift {
                    circle_velocity *= -1.0;
                }
                rigid_body
                    .get_mut(&mut physic_world)
                    .set_velocity(Velocity {
                        linear: direct_velocity + circle_velocity,
                        angular: 0.0,
                    });
            }
        }
    }
}
