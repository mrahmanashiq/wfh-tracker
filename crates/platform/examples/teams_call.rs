//! Prints whether a Teams call holds the microphone right now, for checking
//! the sensor by hand: `cargo run -p wt-platform --example teams_call`.

use wt_core::CallSensor;
use wt_platform::TeamsCallSensor;

fn main() {
    println!("teams call: {}", TeamsCallSensor.in_call());
}
