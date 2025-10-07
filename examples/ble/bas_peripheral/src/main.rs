//! A bluetooth battery service example built using Embassy and trouBLE.

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::{join::join, select::select};
use embassy_time::Timer;
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::interrupt::software::SoftwareInterruptControl;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_radio::ble::controller::BleConnector;
use log::{info, warn};
use static_cell::StaticCell;
use trouble_host::prelude::*;
use core::time::Duration;
use embassy_time::Instant;
use bytemuck::{bytes_of, cast};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_s: Spawner) {
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

    static RADIO: StaticCell<esp_radio::Controller<'static>> = StaticCell::new();
    let radio = RADIO.init(esp_radio::init().unwrap());

    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(radio, bluetooth, Default::default()).unwrap();
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    ble_bas_peripheral_run(controller).await;
}

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;
/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

// GATT Server definition
//#[gatt_server]
//struct Server {
//    battery_service: UartService,
//}
// GATT Server definition
#[gatt_server]
struct Server {
    progressor_service: ProgressorService,
}

#[gatt_service(uuid = "7e4e1701-1ea6-40c9-9dcc-13d34ffead57")]
struct ProgressorService {
    /// Data Point (Notify)
    #[characteristic(
        uuid = "7e4e1702-1ea6-40c9-9dcc-13d34ffead57",
        notify,
        read,
        value = [0; 10] // dummy initial value
    )]
    data_point: [u8; 10],

    /// Control Point (Write / Write Without Response)
    #[characteristic(
        uuid = "7e4e1703-1ea6-40c9-9dcc-13d34ffead57",
        write,
        write_without_response,
        read,
        value = [0]
    )]
    control_point: [u8; 1],
}

/// Battery service
#[gatt_service(uuid = service::BATTERY)]
struct BatteryService {
    /// Battery Level
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0, 100])]
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, name = "hello", read, value = "Battery Level")]
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 10)]
    level: u8,
    #[characteristic(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E", write, read, notify)]
    status: bool,
}

#[gatt_service(uuid = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E")]
struct UartService {
    /// UART TX characteristic
    #[descriptor(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", read)]
    #[characteristic(uuid = "6E400003-B5A3-F393-E0A9-E50E24DCCA9E", read, notify, value = 0)]
    tx: u8,  // or a suitable type for TX data

    /// UART RX characteristic
    #[descriptor(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E")]
    #[characteristic(uuid = "6E400002-B5A3-F393-E0A9-E50E24DCCA9E", write, value = 0)]
    rx: u8,  // or a suitable type for RX data
}

/// Run the BLE stack.
pub async fn ble_bas_peripheral_run<C>(controller: C)
where
    C: Controller,
{
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "TrouBLE",
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    let _ = join(ble_task(runner), async {
        loop {
            match advertise("Progressor_1234", &mut peripheral, &server).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't
                    // run when no one is connected.
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task(&server, &conn, &stack);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    select(a, b).await;
                }
                Err(e) => {
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
///
/// ## Alternative
///
/// If you didn't require this to be generic for your application, you could statically spawn this
/// with i.e.
///
/// ```rust,ignore
/// 
/// #[embassy_executor::task]
/// async fn ble_task(mut runner: Runner<'static, SoftdeviceController<'static>>) {
///     runner.run().await;
/// }
///
/// spawner.must_spawn(ble_task(runner));
/// ```
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
//async fn gatt_events_task<P: PacketPool>(
//    server: &Server<'_>,
//    conn: &GattConnection<'_, '_, P>,
//) -> Result<(), Error> {
//    let level = server.battery_service.tx;
//    let reason = loop {
//        match conn.next().await {
//            GattConnectionEvent::Disconnected { reason } => break reason,
//            GattConnectionEvent::Gatt { event } => {
//                match &event {
//                    GattEvent::Write(event) => {
//                       if event.handle() == level.handle {
//                            info!(
//                                "[gatt] Write Event to Level Characteristic: {:?}",
//                                event.data()
//                            );
//                        }
//                    }
//                    _ => {}
 //               };
 //               // This step is also performed at drop(), but writing it explicitly is necessary
//                // in order to ensure reply is sent.
//                match event.accept() {
//                    Ok(reply) => reply.send().await,
//                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
//                };
//            }
//            _ => {} // ignore other Gatt Connection Events
//        }
//    };
//    info!("[gatt] disconnected: {:?}", reason);
//    Ok(())
//}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let data_point = server.progressor_service.data_point;
    let control_point = server.progressor_service.control_point;

    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Write(e) if e.handle() == control_point.handle => {
                        let data = e.data();
                        log::info!("[gatt] Control Point Write: {:?}", data);

                        if data.len() == 1 && data[0] == 112 {
                            // Build TLV response with opcode 0, length 1, value 42
                            let response_small = [0u8, 1u8, 42u8];
                            let mut response_buf = [0u8; 10];          // characteristic-sized buffer
                            response_buf[..response_small.len()].copy_from_slice(&response_small);
                            log::info!("[gatt] Sending response: {:?}", response_small);

                            if data_point.notify(conn, &response_buf).await.is_err() {
                                log::warn!("[gatt] Failed to notify data point");
                            }
                        }
                    }
                    GattEvent::Read(e) if e.handle() == data_point.handle => {
                        let value = server.get(&data_point);
                        log::info!("[gatt] Data Point Read: {:?}", value);
                    }
                    _ => {}
                }

                if let Ok(reply) = event.accept() {
                    reply.send().await;
                }
            }
            _ => {}
        }
    };

    log::info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}


/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids16(&[[0x0f, 0x18]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?.with_attribute_server(server)?;
    info!("[adv] connection established");
    Ok(conn)
}

/// Example task to use the BLE notifier interface.
/// This task will notify the connected central of a counter value every 2 seconds.
/// It will also read the RSSI value every 2 seconds.
/// and will stop when the connection is closed by the central or an error occurs.
//async fn custom_task<C: Controller, P: PacketPool>(
//    server: &Server<'_>,
//    conn: &GattConnection<'_, '_, P>,
//    stack: &Stack<'_, C, P>,
//) {
//    let mut tick: u8 = 0;
//    let level = server.battery_service.tx;
//    loop {
//        //tick = tick.wrapping_add(1);
//        tick = b'C';
//        info!("[custom_task] notifying connection of tick {}", tick);
//        if level.notify(conn, &tick).await.is_err() {
//            info!("[custom_task] error notifying connection");
//            break;
//        };
//        // read RSSI (Received Signal Strength Indicator) of the connection.
//        if let Ok(rssi) = conn.raw().rssi(stack).await {
//            info!("[custom_task] RSSI: {:?}", rssi);
 //       } else {
//            info!("[custom_task] error getting RSSI");
//            break;
//        };
//        Timer::after_millis(100).await;
//    }
//}
//async fn custom_task<C: Controller, P: PacketPool>(
//    server: &Server<'_>,
//    conn: &GattConnection<'_, '_, P>,
//    _stack: &Stack<'_, C, P>,
//) {
//    let mut tick = [0; 3];
//    let data_point = server.progressor_service.data_point;
//
//    loop {
//        tick = [0; 3];
//        if data_point.notify(conn, &tick).await.is_err() {
//            log::info!("[custom_task] notify failed");
//            break;
//        }
//        Timer::after_secs(1).await;
//    }
//}

async fn custom_task<C: Controller, P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    _stack: &Stack<'_, C, P>,
) {
    let data_point = server.progressor_service.data_point;
    let start = Instant::now();
    let mut weight: f32 = 0.0;

    loop {
        // Simulate weight measurement (you could also read from a sensor here)
        weight += 0.5;
        if weight > 50.0 {
            weight = 0.0;
        }

        // Calculate timestamp (microseconds since connection started)
        let timestamp_us: u32 = start.elapsed().as_micros() as u32;

        // Build response payload
        // [0x01][0x08][weight(float32)][timestamp(uint32)]
        let mut packet = [0u8; 10];
        packet[0] = 0x01; // response code
        packet[1] = 0x08; // length (4 bytes weight + 4 bytes timestamp)
        packet[2..6].copy_from_slice(bytes_of(&weight));
        packet[6..10].copy_from_slice(bytes_of(&timestamp_us));

        log::info!(
            "[custom_task] sending weight={} timestamp={}us packet={:x?}",
            weight,
            timestamp_us,
            &packet
        );

        if data_point.notify(conn, &packet).await.is_err() {
            log::warn!("[custom_task] notify failed - connection probably closed");
            break;
        }

        embassy_time::Timer::after_millis(100).await;
    }
}
