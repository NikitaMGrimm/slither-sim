//! Authored numeric visibility/selection rules; no sockets, browser or images.
use slither_client::{
    presentation::*,
    state::{Direction, Point, PreySpawnInput},
};
use std::borrow::Cow;

fn asset(handle: u64) -> BodyAsset {
    BodyAsset {
        handle,
        revision: 1,
        width: 64,
        height: 64,
        status: BodyAssetStatus::Ready,
    }
}
fn view() -> BodyPaintView {
    BodyPaintView {
        camera: Point::default(),
        zoom: 1.,
        half_width: 100.,
        half_height: 80.,
    }
}
fn food_input<'a>(foods: &'a [FoodPresentation], assets: &'a [BodyAsset]) -> FoodPaintInput<'a> {
    FoodPaintInput {
        foods,
        view: view(),
        quality: BodyPaintQuality {
            high_quality: true,
            blend: 1.,
        },
        bounds: Some(FoodCullBounds::from_view(view()).unwrap()),
        assets,
        target_handle: None,
        entry_alpha: 1.,
        command_limit: MAX_FOOD_PAINT_COMMANDS,
    }
}
fn prey_input<'a>(preys: &'a [PreyPresentation], assets: &'a [BodyAsset]) -> PreyPaintInput<'a> {
    PreyPaintInput {
        preys,
        eaters: &[],
        view: view(),
        assets,
        target_handle: None,
        early_entry_alpha: 1.,
        late_entry_alpha: 1.,
        command_limit: MAX_PREY_PAINT_COMMANDS,
    }
}
fn food(world: &mut FoodWorld, key: u32, x: f64) {
    let source = FoodSource::new(asset(1), 64., 64., 32., 32.).unwrap();
    world
        .spawn(
            FoodSpawn {
                key,
                sector: (0, 0),
                position: Point { x, y: 0. },
                base_size: 0.,
                raw_color: 0,
                rapid: true,
            },
            FoodFactoryInput {
                random_phase_unit: 0.,
                random_wobble_unit: 0.,
                creation_zoom: 1.,
                palette: FoodPalette {
                    palette: 0,
                    image_count: 1,
                    bodies: &[source],
                    outlines: &[source],
                    glows: &[source],
                },
            },
        )
        .unwrap();
}
fn prey(world: &mut PreyWorld, id: u16, x: f64) {
    let source = PreySource::new(asset(2), 64., 64., 32., 32.).unwrap();
    let factory = PreyFactory {
        random_phase_unit: 0.,
        random_rate_unit: 0.,
        palette: 0,
        bodies: vec![source],
        glows: vec![source],
    };
    world
        .spawn(
            PreySpawnInput {
                id,
                absolute_position: Point { x, y: 0. },
                size_byte: 0,
                base_size: 0.,
                raw_color: 0,
                direction: Direction::None,
                heading: 0.,
                wanted_heading: 0.,
                speed_per_32ms: 0.,
            },
            factory.input(1.),
        )
        .unwrap();
}

#[test]
fn pressure_food_culls_before_quota_and_both_planner_paths_keep_the_same_set() {
    let bounds = Some(FoodCullBounds::from_view(view()).unwrap());
    for seed in [7, 31, 97] {
        let mut world = FoodWorld::new();
        for i in 0..32 {
            food(
                &mut world,
                i,
                if (i * 17 + seed) % 5 == 0 {
                    f64::from(i)
                } else {
                    100000.
                },
            );
        }
        for quota in 0..=4 {
            let budget = PressureEntityBudget {
                food_commands: quota * 7,
                prey_commands: 0,
            };
            let canvas = budget.foods(true, world.foods(), bounds, 1.);
            let native = budget.foods(true, world.foods(), bounds, 1.);
            let visible = world
                .foods()
                .iter()
                .filter(|f| f.snapshot().render_position.x < 100.)
                .count();
            assert_eq!(canvas.len(), quota.min(visible));
            assert!(canvas.iter().all(|f| f.snapshot().render_position.x < 100.));
            assert_eq!(canvas, native);
            let assets = [asset(1)];
            let canvas_plan = plan_food(food_input(&canvas, &assets)).unwrap();
            let native_plan = plan_food_with_zoom_and_index(
                food_input(&native, &assets),
                &mut FoodAssetIndex::default(),
                vec![],
                vec![],
                1.,
            )
            .unwrap();
            assert_eq!(canvas_plan, native_plan);
            assert!(matches!(
                budget.foods(false, world.foods(), bounds, 1.),
                Cow::Borrowed(_)
            ));
        }
    }
}

#[test]
fn pressure_prey_keeps_older_bodies_and_glows_before_offscreen_newer_entities() {
    let mut world = PreyWorld::new();
    prey(&mut world, 1, 0.);
    prey(&mut world, 2, 200.);
    // The raw body is invisible, but its eaten glow has moved into view.
    prey(&mut world, 12, 500.);
    let token = SnakeObjectToken::new(1).unwrap();
    let pose = PreyEaterPose {
        head: Point::default(),
        head_offset: Point::default(),
        heading: 0.,
        angle_offset: 0.,
    };
    world.consume(12, Some(token));
    for _ in 0..8 {
        let step = FrameClock::new(0.)
            .unwrap()
            .advance(40., false, false)
            .unwrap();
        world.advance(step, 0., |_| Ok(pose)).unwrap();
    }
    let eaters = [(token, pose)];
    for id in 3..12 {
        prey(&mut world, id, 100000.);
    }
    let budget = PressureEntityBudget {
        food_commands: 0,
        prey_commands: 18,
    };
    let canvas = budget.preys(true, world.preys(), &eaters, view()).unwrap();
    let native = budget.preys(true, world.preys(), &eaters, view()).unwrap();
    assert_eq!(canvas, native);
    assert_eq!(canvas.len(), 3);
    assert!(canvas.iter().all(|p| p.snapshot().position.x <= 500.));
    let assets = [asset(2)];
    let canvas_input = PreyPaintInput {
        eaters: &eaters,
        ..prey_input(&canvas, &assets)
    };
    let native_input = PreyPaintInput {
        eaters: &eaters,
        ..prey_input(&native, &assets)
    };
    assert_eq!(
        plan_prey(canvas_input).unwrap(),
        plan_prey_with_zoom(native_input, 1.).unwrap()
    );
    assert!(matches!(
        budget.preys(false, world.preys(), &[], view()).unwrap(),
        Cow::Borrowed(_)
    ));
}
