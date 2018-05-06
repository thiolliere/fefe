use specs::{Join, World};

pub trait GameState {
    fn update_draw_ui(self: Box<Self>, world: &mut World) -> Box<GameState>;
    fn winit_event(
        self: Box<Self>,
        event: ::winit::Event,
        world: &mut World,
    ) -> Box<GameState>;
    fn gilrs_event(
        self: Box<Self>,
        event: ::gilrs::EventType,
        world: &mut World,
    ) -> Box<GameState>;
    fn gilrs_gamepad_state(
        self: Box<Self>,
        id: usize,
        gamepad: &::gilrs::Gamepad,
        world: &mut World,
    ) -> Box<GameState>;
    fn quit(&self) -> bool {
        false
    }
    fn paused(&self) -> bool;
}

pub struct Game;

impl GameState for Game {
    fn update_draw_ui(self: Box<Self>, _world: &mut World) -> Box<GameState> {
        self
    }
    fn winit_event(
        self: Box<Self>,
        _event: ::winit::Event,
        _world: &mut World,
    ) -> Box<GameState> {
        self
    }

    fn gilrs_event(
        self: Box<Self>,
        _event: ::gilrs::EventType,
        _world: &mut World,
    ) -> Box<GameState> {
        self
    }

    fn gilrs_gamepad_state(
        self: Box<Self>,
        _id: usize,
        gamepad: &::gilrs::Gamepad,
        world: &mut World,
    ) -> Box<GameState> {
        let px = gamepad.axis_data(::gilrs::Axis::LeftStickX).map(|e| e.value());
        let py = gamepad.axis_data(::gilrs::Axis::LeftStickY).map(|e| e.value());

        if let (Some(px), Some(py)) = (px, py) {
            for (_, rigid_body) in (
                &world.read::<::component::Player>(),
                &mut world.write::<::component::RigidBody>(),
            ).join() {
                rigid_body.get_mut(&mut world.write_resource()).set_velocity(
                    ::nphysics2d::math::Velocity {
                        linear: ::na::Vector2::new(px, py)*::CFG.player_velocity,
                        angular: 0.0,
                    }
                );
            }
        }

        let ax = gamepad.axis_data(::gilrs::Axis::RightStickX).map(|e| e.value());
        let ay = gamepad.axis_data(::gilrs::Axis::RightStickY).map(|e| e.value());

        if let (Some(ax), Some(ay)) = (ax, ay) {
            for (_, aim) in (
                &world.read::<::component::Player>(),
                &mut world.write::<::component::Aim>(),
            ).join() {
                **aim = ay.atan2(ax);
            }
        }

        self
    }

    fn paused(&self) -> bool {
        false
    }
}
