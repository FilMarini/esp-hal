use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use crate::datapoint::DataOpcode;

pub(crate) const DATA_PAYLOAD_SIZE: usize = crate::datapoint::DATA_PAYLOAD_SIZE;

#[derive(Copy, Clone, Debug)]
pub enum MeasurementCommand {
    Start,
    Stop,
    Tare,
}

pub static MEASUREMENT_CMD: Signal<CriticalSectionRawMutex, MeasurementCommand> = Signal::new();

// channel to send measurement data to BLE task
use embassy_sync::channel::Channel;
pub static MEASUREMENT_DATA: Channel<CriticalSectionRawMutex, DataOpcode, 4> = Channel::new();

mod task;
pub use task::start_measurement_task;

