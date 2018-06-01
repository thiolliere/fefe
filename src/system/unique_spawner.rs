use specs::{Join, Fetch, ReadStorage, System, WriteStorage};
use entity::Insertable;
use ncollide2d::world::CollisionGroups;
use ncollide2d::query::Ray;
use rand::thread_rng;
use rand::distributions::{IndependentSample, Range};
use std::f32::consts::PI;

pub struct UniqueSpawnerSystem;

impl<'a> System<'a> for UniqueSpawnerSystem {
    type SystemData = (
        ReadStorage<'a, ::component::Activator>,
        ReadStorage<'a, ::component::Aim>,
        ReadStorage<'a, ::component::Player>,
        ReadStorage<'a, ::component::RigidBody>,
        WriteStorage<'a, ::component::UniqueSpawner>,
        Fetch<'a, ::resource::PhysicWorld>,
        Fetch<'a, ::resource::LazyUpdate>,
        Fetch<'a, ::resource::EntitiesRes>,
        Fetch<'a, ::resource::BodiesMap>,
    );

    fn run(&mut self, (activators, aims, players, bodies, mut unique_spawners, physic_world, lazy_update, entities, bodies_map): Self::SystemData) {
        let mut rng = thread_rng();
        let range_0_1 = Range::new(0.0, 1.0);

        let players_aim = (&players, &aims, &bodies)
            .join()
            .map(|(_, aim, body)| {
                (aim.0, body.get(&physic_world).position().translation.vector)
            })
            .collect::<Vec<_>>();

        for (unique_spawner, body, activator, entity) in (&mut unique_spawners, &bodies, &activators, &*entities).join() {
            if activator.activated {
                let position = body.get(&physic_world).position().clone();
                let pos_vector = position.translation.vector;
                for &(player_aim, ref player_position) in &players_aim {
                    let dist_vector = player_position - pos_vector;

                    let mut proba = 1.0;
                    if let Some(ref dist_proba_clamp) = unique_spawner.dist_proba_clamp {
                        let distance = dist_vector.norm();
                        proba *= dist_proba_clamp.compute(distance);
                    }
                    if let Some(ref aim_proba_clamp) = unique_spawner.aim_proba_clamp {
                        let mut v = pos_vector - player_position;
                        let angle = v[1].atan2(v[0]);
                        let mut angle_distance = (angle - player_aim).abs() % 2.0 * PI;
                        if angle_distance >= PI {
                            angle_distance = 2.0 * PI - angle_distance;
                        }
                        proba *= aim_proba_clamp.compute(angle_distance);
                    }

                    if range_0_1.ind_sample(&mut rng) <= proba {
                        let ray = Ray::new(::na::Point::from_coordinates(pos_vector), dist_vector);
                        let mut collision_groups = CollisionGroups::new();
                        collision_groups.set_whitelist(&[
                            ::entity::Group::Wall as usize,
                            ::entity::Group::Player as usize,
                        ]);
                        let interference = physic_world.collision_world().interferences_with_ray(&ray, &collision_groups)
                            .min_by_key(|(_, intersection)| (intersection.toi * ::CMP_PRECISION) as isize);
                        if let Some((object, _)) = interference {
                            if players.get(*bodies_map.get(&object.data().body()).unwrap()).is_some() {
                                entities.delete(entity).unwrap();
                                let spawn_entity = unique_spawner.entity.clone();
                                lazy_update.execute(move |world| {
                                    spawn_entity.insert(position.into(), world);
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
