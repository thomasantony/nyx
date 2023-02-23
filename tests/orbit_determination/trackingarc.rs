use nyx_space::io::stations::StationSerde;
use nyx_space::io::tracking_data::DynamicTrackingArc;
use nyx_space::io::{ConfigRepr, Configurable};
use nyx_space::md::ui::*;
use nyx_space::od::msr::StdMeasurement;
use nyx_space::od::simulator::arc::TrackingArcSim;
use nyx_space::od::ui::*;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;

#[test]
fn tracking_arc_simple() {
    if pretty_env_logger::try_init().is_err() {
        println!("could not init env_logger");
    }

    // Load cosm
    let cosm = Cosm::de438();

    // Dummy state
    let orbit = Orbit::keplerian_altitude(
        500.0,
        1e-3,
        30.0,
        45.0,
        75.0,
        23.4,
        Epoch::from_str("2023-02-22T19:18:17.16 UTC").unwrap(),
        cosm.frame("EME2000"),
    );
    // Generate a trajectory
    let (_, trajectory) = Propagator::default(OrbitalDynamics::two_body())
        .with(orbit)
        .for_duration_with_traj(1.5.days())
        .unwrap();

    println!("{trajectory}");

    // Load the ground stations from the test data.
    let ground_station_yaml: PathBuf = [
        &env::var("CARGO_MANIFEST_DIR").unwrap(),
        "data",
        "tests",
        "config",
        "many_ground_stations.yaml",
    ]
    .iter()
    .collect();

    let stations_serde = StationSerde::load_many_yaml(ground_station_yaml).unwrap();
    let devices: Vec<GroundStation> = stations_serde
        .iter()
        .map(|station| GroundStation::from_config(&station, cosm.clone()).unwrap())
        .collect();

    // Build the tracking arc simulation to generate a "standard measurement".
    let mut trk = TrackingArcSim::<_, StdMeasurement, _>::with_seed(devices, trajectory, 12345);

    let arc = trk.generate_measurements(cosm).unwrap();

    assert_eq!(arc.measurements.len(), 4322);

    // And serialize to disk
    let path: PathBuf = [
        &env::var("CARGO_MANIFEST_DIR").unwrap(),
        "output_data",
        "simple_arc.parquet",
    ]
    .iter()
    .collect();

    let output_fn = arc.write_parquet(path).unwrap();
    println!("[{}] {arc}", output_fn.to_string_lossy());

    // Now read this file back in.
    let dyn_arc = DynamicTrackingArc::from_parquet(output_fn).unwrap();
    // And convert to the same tracking arc as earlier
    let arc_concrete = dyn_arc.to_tracking_arc::<StdMeasurement>().unwrap();

    dbg!(arc_concrete);
}
