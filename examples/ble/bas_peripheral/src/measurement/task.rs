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
use crate::calibration_mem::CalibrationMem;

#[task]
pub async fn start_measurement_task(load_sensor: &'static Mutex<CriticalSectionRawMutex, HX711<Output<'static>, Input<'static>, Delay>>) {
    let mut start = Instant::now();
    loop {
        match MEASUREMENT_CMD.wait().await {
            MeasurementCommand::Start => {
                start = Instant::now();
                loop {
                    if let Some(MeasurementCommand::Stop) = MEASUREMENT_CMD.try_take() {
                        break;
                    }
                    let mut sensor = load_sensor.lock().await;
                    if sensor.is_ready() {
                        if let Ok(weight) = sensor.read_scaled() {
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
                debug_info("Taring the load sensor");
                let mut sensor = load_sensor.lock().await;
                sensor.tare(16);
            }
        }
    }
}

#[task]
pub async fn run_calibration(load_sensor: &'static Mutex<CriticalSectionRawMutex, HX711<Output<'static>, Input<'static>, Delay>>, mut cal_mem: CalibrationMem<'static>, mut button: Input<'static>, delay: Delay, millis: u32) {
    loop {
        if press_for_millis(&button, &delay, millis) {
            debug_info("Entering calibration..");
            let mut sensor = load_sensor.lock().await;
            let first_meas = sensor.read().unwrap();
            button.wait_for_falling_edge().await;
            let sec_meas = sensor.read().unwrap();
            let cal_value: f32 = 10.0 / ((sec_meas - first_meas) as f32);
            debug_info(&format!("Calibration value is {:?}", cal_value));
            cal_mem.set_calibration(cal_value);
        } else {
            Timer::after_millis(100).await;
        }
    }
}
