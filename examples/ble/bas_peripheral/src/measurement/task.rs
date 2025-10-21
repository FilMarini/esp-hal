use embassy_executor::task;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, Output};
use loadcell::hx711::HX711;
use loadcell::LoadCell;
use alloc::format;
use crate::datapoint::DataOpcode;
use crate::measurement::{MeasurementCommand, MEASUREMENT_CMD, MEASUREMENT_DATA};
use crate::utils::*;
use super::HX711BB;

#[task]
pub async fn start_measurement_task(load_sensor: &'static Mutex<CriticalSectionRawMutex, HX711BB<'static, Output<'static>, Input<'static>, Delay>>) {
    loop {
        match MEASUREMENT_CMD.wait().await {
            MeasurementCommand::Start => {
                {
                    let mut sensor = load_sensor.lock().await;
                    sensor.start_now();
                }
                loop {
                    if let Some(MeasurementCommand::Stop) = MEASUREMENT_CMD.try_take() {
                        break;
                    }
                    let mut sensor = load_sensor.lock().await;
                    if let packet = sensor.get_weight_packet() {
                        MEASUREMENT_DATA.send(packet).await;
                    }
                }
                    Timer::after_millis(10).await;
            }
            MeasurementCommand::Stop => {}
            MeasurementCommand::Tare => {
                debug_info("Taring the load sensor");
                let mut sensor = load_sensor.lock().await;
                sensor.tare(16);
            }
        }
    }
}

#[task]
pub async fn run_calibration(load_sensor: &'static Mutex<CriticalSectionRawMutex, HX711BB<'static, Output<'static>, Input<'static>, Delay>>, mut button: Input<'static>, mut led: Output<'static>, millis: u32) {
    loop {
        if press_for_millis(&button, millis) {
            led.set_high();
            debug_info("Entering calibration..");
            let mut sensor = load_sensor.lock().await;
            sensor.init_calibration();
            button.wait_for_rising_edge().await;
            Timer::after_millis(3).await;
            button.wait_for_falling_edge().await;
            Timer::after_millis(500).await;
            sensor.calibrate();
            led.set_low();
            Timer::after_millis(1000).await;
        } else {
            Timer::after_millis(100).await;
        }
    }
}
