use bytemuck::{bytes_of, Pod, Zeroable};
use bytemuck_derive::{Pod, Zeroable};
use defmt::Format;

pub(crate) const DATA_PAYLOAD_SIZE: usize = 12;

/// DataOpCode: Data to send in response to ControlOpcode
#[derive(Copy, Clone)]
pub(crate) enum DataOpcode {
    BatteryVoltage(u32), // Not currently supported
    Weight(f32, u32),
    LowPowerWarning, // Not currently supported
    AppVersion(&'static [u8]),
    ProgressorId(u8),
}

impl DataOpcode {
    fn opcode(&self) -> u8 {
        match self {
            DataOpcode::BatteryVoltage(..)
                | DataOpcode::AppVersion(..)
                | DataOpcode::ProgressorId(..) => 0x00,
            DataOpcode::Weight(..) => 0x01,
            DataOpcode::LowPowerWarning => 0x04,
        }
    }

    fn length(&self) -> u8 {
        match self {
            DataOpcode::BatteryVoltage(..) => 4,
            DataOpcode::Weight(..) => 8,
            DataOpcode::ProgressorId(..) => 1,
            DataOpcode::LowPowerWarning => 0,
            DataOpcode::AppVersion(version) => version.len() as u8,
        }
    }

    fn value(&self) -> [u8; DATA_PAYLOAD_SIZE] {
        let mut value = [0; DATA_PAYLOAD_SIZE];
        match self {
            DataOpcode::BatteryVoltage(voltage) => {
                value[0..4].copy_from_slice(&voltage.to_le_bytes());
            }
            DataOpcode::Weight(weight, timestamp) => {
                value[0..4].copy_from_slice(&weight.to_le_bytes());
                value[4..8].copy_from_slice(&timestamp.to_le_bytes());
            }
            DataOpcode::LowPowerWarning => (),
            DataOpcode::ProgressorId(id) => {
                value[0..1].copy_from_slice(&id.to_le_bytes());
            }
            DataOpcode::AppVersion(version) => {
                value[0..version.len()].copy_from_slice(version);
            }
        };
        value
    }

    pub(crate) fn to_bytes(&self) -> [u8; DATA_PAYLOAD_SIZE + 2] {
        let mut buf = [0u8; DATA_PAYLOAD_SIZE + 2];
        buf[0] = self.opcode();
        buf[1] = self.length();
        buf[2..].copy_from_slice(&self.value());
        buf
    }

}

/// ControlOpcode: command received
#[derive(Copy, Clone)]
pub(crate) enum ControlOpcode {
    Tare,
    StartMeasurement,
    StopMeasurement,
    StartPeakRfdMeasurement,
    StartPeakRfdMeasurementSeries,
    GetAppVersion,
    GetErrorInfo,
    ClearErrorInfo,
    SampleBattery,
    GetProgressorID,
    Unknown(u8),
    Invalid,
}

impl ControlOpcode {
    pub(crate) fn is_known_opcode(&self) -> bool {
        !matches!(self, Self::Unknown(_) | Self::Invalid)
    }
    pub(crate) fn from_bytes(data: &[u8]) -> Self {
        if data.is_empty() {
            return ControlOpcode::Invalid;
        }

        match data[0] {
            0x00 => ControlOpcode::Tare,
            0x65 => ControlOpcode::StartMeasurement,
            0x66 => ControlOpcode::StopMeasurement,
            0x03 => ControlOpcode::StartPeakRfdMeasurement,
            0x04 => ControlOpcode::StartPeakRfdMeasurementSeries,
            0x05 => ControlOpcode::GetAppVersion,
            0x06 => ControlOpcode::GetErrorInfo,
            0x07 => ControlOpcode::ClearErrorInfo,
            0x09 => ControlOpcode::SampleBattery,
            0x70 => ControlOpcode::GetProgressorID,
            other => ControlOpcode::Unknown(other),
        }
    }
}
