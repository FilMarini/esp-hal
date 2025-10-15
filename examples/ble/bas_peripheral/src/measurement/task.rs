use embassy_executor::task;
use embassy_sync::signal::Signal;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use esp_hal::delay::Delay;
use loadcell::hx711::HX711;
use loadcell::LoadCell;
use log::{info, warn};
use crate::datapoint::DataOpcode;
use crate::measurement::{MeasurementCommand, MEASUREMENT_CMD, MEASUREMENT_DATA};

#[task]
pub async fn start_measurement_task(mut load_sensor: HX711<esp_hal::gpio::Output<'static>, esp_hal::gpio::Input<'static>, Delay>) {
    let mut start = Instant::now();

    loop {
        match MEASUREMENT_CMD.wait().await {
            MeasurementCommand::Start => {
                start = Instant::now();
                loop {
                    if let Some(MeasurementCommand::Stop) = MEASUREMENT_CMD.try_take() {
                        break;
                    }

                    if load_sensor.is_ready() {
                        if let Ok(weight) = load_sensor.read_scaled() {
                            let timestamp = start.elapsed().as_micros() as u32;
                            let packet = DataOpcode::Weight(weight, timestamp);
                            MEASUREMENT_DATA.send(packet).await;
                        }
                    }

                    Timer::after_millis(10).await;
                }
            }
            MeasurementCommand::Stop => {}
            MeasurementCommand::Tare => {
                info!{"Taring the load sensor"}
                load_sensor.tare(16);
            }
        }
    }
}
