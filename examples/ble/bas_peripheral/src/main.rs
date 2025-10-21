#![no_std]
#![no_main]

extern crate alloc;

mod ble;
mod measurement;
mod datapoint;
mod utils;
mod calibration_mem;

use embassy_executor::Spawner;
use esp_hal::{clock::CpuClock, delay::Delay, gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull}, timer::timg::TimerGroup};
use esp_alloc as _;
use esp_backtrace as _;
use esp_storage::FlashStorage;
use alloc::format;

#[cfg(target_arch = "riscv32")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use loadcell::{hx711, LoadCell};
use utils::debug_info;
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use static_cell::StaticCell;
use measurement::{HX711BB, LOAD_SENSOR};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    #[cfg(target_arch = "riscv32")]
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    esp_rtos::start(
        timg0.timer0,
        #[cfg(target_arch = "riscv32")]
        sw_int.software_interrupt0,
    );

    // --- GPIO Setup ---
    let hx711_sck = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let hx711_dt = Input::new(peripherals.GPIO6, InputConfig::default());
    let calib_config = InputConfig::default().with_pull(Pull::Up);
    let calib_button = Input::new(peripherals.GPIO9, calib_config);
    // --- Delay Setup ---
    let delay = Delay::new();
    // --- Peripherals Setup ---
    let mut load_sensor = hx711::HX711::new(hx711_sck, hx711_dt, delay);
    let mut flash = FlashStorage::new(peripherals.FLASH);
    let mut cal_mem = calibration_mem::CalibrationMem::new(flash);
    let mut load_sensor_bb = HX711BB::new(load_sensor, cal_mem);

    embassy_time::Timer::after_millis(3000).await;
    while !load_sensor_bb.is_ready() {
        debug_info("Waiting for HX711 to power up");
        embassy_time::Timer::after_millis(1000).await;
    }
    load_sensor_bb.set_scale_from_memory();
    load_sensor_bb.tare(32);
    debug_info("Load sensor tared!");
    let shared_sensor = LOAD_SENSOR.init(Mutex::new(load_sensor_bb));

    // --- Start Measurement and Calibration Task ---
    spawner.spawn(measurement::start_measurement_task(shared_sensor)).unwrap();
    spawner.spawn(measurement::run_calibration(shared_sensor, calib_button, 2000u32)).unwrap();

    // --- BLE Setup ---
    ble::run_ble(peripherals.BT).await;
}
