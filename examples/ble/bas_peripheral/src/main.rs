#![no_std]
#![no_main]

extern crate alloc;

mod ble;
mod measurement;
mod datapoint;

use embassy_executor::Spawner;
use esp_hal::{clock::CpuClock, delay::Delay, gpio::{Input, InputConfig, Level, Output, OutputConfig}, timer::timg::TimerGroup};
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use loadcell::{hx711, LoadCell};
use log::{info, warn};

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

    // --- Load Cell Setup ---
    let hx711_sck = Output::new(peripherals.GPIO5, Level::Low, OutputConfig::default());
    let hx711_dt = Input::new(peripherals.GPIO6, InputConfig::default());
    let delay = Delay::new();

    let mut load_sensor = hx711::HX711::new(hx711_sck, hx711_dt, delay);
    embassy_time::Timer::after_millis(1000).await;
    while !load_sensor.is_ready() {
        info!{"Waiting for HX711 to power up"}
        embassy_time::Timer::after_millis(1000).await;
    }
    load_sensor.tare(32);
    #[cfg(debug_assertions)]
    info!("Load sensor tared!");
    load_sensor.set_scale(0.0001);

    // --- Start Measurement Task ---
    spawner.spawn(measurement::start_measurement_task(load_sensor)).unwrap();

    // --- BLE Setup ---
    ble::run_ble(peripherals.BT).await;
}
