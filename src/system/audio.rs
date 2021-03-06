use specs::Join;

pub struct AudioSystem;

impl<'a> ::specs::System<'a> for AudioSystem {
    type SystemData = (
        ::specs::ReadStorage<'a, ::component::Player>,
        ::specs::ReadStorage<'a, ::component::RigidBody>,
        ::specs::ReadExpect<'a, ::resource::PhysicWorld>,
        ::specs::ReadExpect<'a, ::resource::Save>,
        ::specs::WriteExpect<'a, ::resource::Audio>,
    );

    fn run(
        &mut self,
        (players, bodies, physic_world, save, mut audio): Self::SystemData,
    ) {
        // TODO: Fix it when multiple bodies
        let position = (&players, &bodies).join().next().map(|(_, body)| {
            body.get(&physic_world)
                .position()
                .translation.vector
        });

        audio.update(position, &save);
    }
}
