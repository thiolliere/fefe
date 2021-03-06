extern crate alga;
#[macro_use]
extern crate imgui;
#[macro_use]
extern crate derive_deref;
#[macro_use]
extern crate failure;
extern crate fnv;
extern crate fps_counter;
extern crate gilrs;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate lyon;
extern crate nalgebra as na;
extern crate ncollide2d;
extern crate nphysics2d;
extern crate png;
extern crate rand;
extern crate ron;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate specs;
#[macro_use]
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;
extern crate hibitset;
extern crate vulkano_win;
extern crate winit;
#[macro_use]
extern crate specs_derive;
extern crate rodio;
extern crate show_message;
extern crate heck;
extern crate enum_iterator;
#[macro_use]
extern crate enum_iterator_derive;
extern crate app_dirs2;

mod dynamic_mixer;
mod audio;
pub mod animation;
pub mod component;
mod config_menu;
pub mod entity;
mod force_generator;
pub mod map;
mod resource;
mod system;
#[macro_use]
mod util;
mod game_state;
mod graphics;
mod retained_storage;

pub use resource::Conf;
pub use util::ClampFunction;

use game_state::GameState;
use specs::{DispatcherBuilder, World};
use std::thread;
use std::time::Duration;
use std::time::Instant;
use vulkano::instance::Instance;
use vulkano_win::VkSurfaceBuild;
use winit::CursorState;

const CMP_PRECISION: f32 = 100.0;

fn main() {
    ::std::env::set_var("WINIT_UNIX_BACKEND", "x11");

    let mut gilrs = gilrs::Gilrs::new().unwrap();

    let instance = {
        let extensions = vulkano_win::required_extensions();
        let info = app_info_from_cargo_toml!();
        Instance::new(Some(&info), &extensions, None).expect("failed to create Vulkan instance")
    };

    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_fullscreen(Some(events_loop.get_primary_monitor()))
        .build_vk_surface(&events_loop, instance.clone())
        .unwrap();

    try_multiple_time!(window.window().set_cursor_state(winit::CursorState::Grab)).unwrap();
    window.window().set_cursor(winit::MouseCursor::NoneCursor);
    window.window().set_title("fefe");

    let mut imgui = ::util::init_imgui();
    let mut graphics = graphics::Graphics::new(&window, &mut imgui);

    let mut world = World::new();
    world.register::<::component::RigidBody>();
    world.register::<::component::AnimationState>();
    world.register::<::component::Ground>();
    world.register::<::component::Life>();
    world.register::<::component::Aim>();
    world.register::<::component::Player>();
    world.register::<::component::GravityToPlayers>();
    world.register::<::component::DeadOnContact>();
    world.register::<::component::ContactDamage>();
    world.register::<::component::Contactor>();
    world.register::<::component::ControlForce>();
    world.register::<::component::PlayersAimDamping>();
    world.register::<::component::PlayersDistanceDamping>();
    world.register::<::component::GravityToPlayers>();
    world.register::<::component::Damping>();
    world.register::<::component::TurretSpawner>();
    world.register::<::component::DebugColor>();
    world.register::<::component::UniqueSpawner>();
    world.register::<::component::ChamanSpawner>();
    world.register::<::component::DebugCircles>();
    world.register::<::component::DebugRays>();
    world.register::<::component::VelocityToPlayerMemory>();
    world.register::<::component::VelocityToPlayerRandom>();
    world.register::<::component::VelocityToPlayerCircle>();
    world.register::<::component::VelocityControl>();
    world.register::<::component::VelocityAimDamping>();
    world.register::<::component::VelocityDistanceDamping>();
    world.register::<::component::PositionInPath>();
    world.register::<::component::VelocityToPlayerCircle>();
    world.register::<::component::Activators>();
    world.register::<::component::Boid>();
    world.register::<::component::SwordRifle>();

    let conf = ::resource::Conf::load();
    let save = ::resource::Save::load();
    world.add_resource(::resource::Audio::init(&conf, &save));
    world.add_resource(::resource::UpdateTime(0.0));
    world.add_resource(::resource::AnimationImages(vec![]));
    world.add_resource(::resource::Camera::new(::na::one(), conf.zoom));
    world.add_resource(::resource::DebugShapes(vec![]));
    world.add_resource(::resource::WindowSize(
        window.window().get_inner_size().unwrap(),
    ));
    world.add_resource(imgui);
    world.add_resource(conf);
    world.add_resource(save);
    world.add_resource(::resource::Tempos(vec![::resource::Tempo::new(0.2)]));
    world.maintain();

    // TOCHECK:
    // * all system checking for activator must depends on it system
    // * velocity dampings must depends on all system setting velcoity
    // * tout ce qui utilise des positions doivent être après physic system
    // * positioner system must be before every system that use position
    let mut update_dispatcher = DispatcherBuilder::new()
        .with(::system::ActivatorSystem, "activator", &[])
        .with(::system::PhysicSystem::new(), "physic", &[])
        .with_barrier()
        .with(::system::PositionInPathSystem, "position in path", &[])
        .with_barrier()
        .with(::system::SwordRifleSystem, "sword rifle", &[])
        .with(::system::DeadOnContactSystem, "dead on contact", &[])
        .with(::system::ContactDamageSystem, "damage", &[])
        .with(::system::UniqueSpawnerSystem, "unique spawner", &["activator"])
        .with(::system::ChamanSpawnerSystem, "chaman spawner", &["activator"])
        .with(::system::TurretSpawnerSystem, "turret spawner", &["activator"])
        .with(::system::VelocityToPlayerMemorySystem, "velocity to player memory", &["activator"])
        .with(::system::VelocityToPlayerRandomSystem, "velocity to player random", &["activator"])
        .with(::system::VelocityToPlayerCircleSystem, "velocity to player circle", &["activator"])
        .with(::system::VelocityControlSystem, "velocity control", &[])
        .with(::system::Boid, "boid", &[])
        .with(::system::VelocityDampingsSystem, "velocity dampings", &[
             "velocity to player memory",
             "velocity to player random",
             "velocity to player circle",
             "velocity control",
             "boid",
        ])
        .with(::system::LifeSystem, "life", &[])
        .with_barrier() // Draw barrier
        .with(::system::AudioSystem, "audio", &[])
        .with(::system::AnimationSystem, "animation", &[])
        .with(::system::CameraSystem, "camera", &[])
        .build();

    let mut fps_counter = fps_counter::FPSCounter::new();
    let mut last_frame_instant = Instant::now();
    let mut last_update_instant = Instant::now();

    let mut game_state = Box::new(game_state::Game::default()) as Box<GameState>;

    let mut mouse_down = [false; 5];

    ::map::load_map("one".into(), &mut world).unwrap();

    'main_loop: loop {
        // Parse events
        let mut evs = vec![];
        events_loop.poll_events(|ev| {
            evs.push(ev);
        });
        for ev in evs {
            match ev {
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::Focused(false),
                    ..
                } => {
                    try_multiple_time!(window.window().set_cursor_state(CursorState::Normal))
                        .unwrap();
                }
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::CursorEntered { .. },
                    ..
                } => {
                    try_multiple_time!(window.window().set_cursor_state(CursorState::Grab))
                        .unwrap();
                }
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::Resized(x, y),
                    ..
                } => {
                    world.write_resource::<::resource::WindowSize>().0 = (x, y);
                }
                winit::Event::WindowEvent {
                    event: ::winit::WindowEvent::CloseRequested,
                    ..
                } => {
                    break 'main_loop;
                }
                _ => (),
            }
            ::util::send_event_to_imgui(&ev, &mut world.write_resource(), &mut mouse_down);
            game_state = game_state.winit_event(ev, &mut world);
        }
        while let Some(ev) = gilrs.next_event() {
            gilrs.update(&ev);
            game_state = game_state.gilrs_event(ev.event, &mut world);
        }
        for (id, gamepad) in gilrs.gamepads() {
            game_state = game_state.gilrs_gamepad_state(id, gamepad, &mut world);
        }

        // Quit
        if game_state.quit() {
            break 'main_loop;
        }

        // Update
        let delta_time = last_update_instant.elapsed();
        last_update_instant = Instant::now();
        world.write_resource::<::resource::UpdateTime>().0 = delta_time
            .as_secs()
            .saturating_mul(1_000_000_000)
            .saturating_add(delta_time.subsec_nanos() as u64)
            as f32 / 1_000_000_000.0;

        update_dispatcher.dispatch(&mut world.res);
        game_state = game_state.update_draw_ui(&mut world);

        // Maintain world
        ::util::safe_maintain(&mut world);

        // Draw
        graphics.draw(&mut world, &window);

        // Sleep
        let elapsed = last_frame_instant.elapsed();
        let frame_duration = {
            let fps = world.read_resource::<::resource::Conf>().fps;
            Duration::new(0, (1_000_000_000.0 / fps as f32) as u32)
        };
        if let Some(to_sleep) = frame_duration.checked_sub(elapsed) {
            thread::sleep(to_sleep);
        }
        last_frame_instant = Instant::now();
        fps_counter.tick();
    }
}
