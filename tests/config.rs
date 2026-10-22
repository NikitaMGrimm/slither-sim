use slither_sim::Config;

#[test]
fn initial_food_allocation_is_bounded() {
    for food_count in [0, 100_000] {
        assert!(
            Config {
                food_count,
                ..Config::default()
            }
            .validate()
            .is_ok()
        );
    }
    for food_count in [100_001, usize::MAX] {
        assert!(
            Config {
                food_count,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
    }
}
