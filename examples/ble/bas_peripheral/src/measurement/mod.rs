use embassy_sync::mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use static_cell::StaticCell;
use crate::datapoint::DataOpcode;
use loadcell::hx711::HX711;
use esp_hal::gpio::{Input, Output};
use esp_hal::delay::Delay;

#[derive(Copy, Clone, Debug)]
pub enum MeasurementCommand {
    Start,
    Stop,
    Tare,
}

// signal to start ot stop weight measurements
pub static MEASUREMENT_CMD: Signal<CriticalSectionRawMutex, MeasurementCommand> = Signal::new();
// channel to send measurement data to BLE task
pub static MEASUREMENT_DATA: Channel<CriticalSectionRawMutex, DataOpcode, 4> = Channel::new();
// StaticCell for load_sensor
pub static LOAD_SENSOR: StaticCell<Mutex<CriticalSectionRawMutex, HX711<Output<'static>, Input<'static>, Delay>>> = StaticCell::new();

mod task;
pub use task::start_measurement_task;
pub use task::run_calibration;

