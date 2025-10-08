use bytemuck::{bytes_of, Pod, Zeroable};
use bytemuck_derive::{Pod, Zeroable};
use defmt::Format;

pub(crate) const DATA_PAYLOAD_SIZE: usize = 12;

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct DataPoint {
    opcode: u8,
    length: u8,
    value: [u8; DATA_PAYLOAD_SIZE],
}

impl DataPoint {
    /// Create a new `DataPoint` from scratch
    ///
    /// One should prefer creating a `DataPoint` from a `DataOpcode` to ensure that the packet is
    /// correctly formed.
    pub(crate) fn weight_from_parts(opcode: u8, length: u8, weight: f32, timestamp: u32) -> Self {

        let f32_bytes = weight.to_le_bytes(); // 4 bytes
        let u32_bytes = timestamp.to_le_bytes(); // 4 bytes
        let mut value = [0u8; DATA_PAYLOAD_SIZE];

        // Copy f32 bytes to the first 4 positions
        value[0..4].copy_from_slice(&f32_bytes);

        // Copy u32 bytes to the next 4 positions
        value[4..8].copy_from_slice(&u32_bytes);

        DataPoint {
            opcode,
            length,
            value,
        }
    }

    pub(crate) fn from_parts<T: Pod>(opcode: u8, length: u8, payload: &T) -> Self {
        let src_bytes = bytes_of(payload);
        let mut value = [0u8; DATA_PAYLOAD_SIZE];

        // Copy as many bytes as fit
        let len = core::cmp::min(src_bytes.len(), DATA_PAYLOAD_SIZE);
        value[..len].copy_from_slice(&src_bytes[..len]);
        DataPoint {
            opcode,
            length,
            value,
        }
    }

    /// Convert the struct into `[u8; DATA_PAYLOAD_SIZE+2]`
    pub(crate) fn to_bytes(&self) -> [u8; DATA_PAYLOAD_SIZE + 2] {
        let mut buf = [0u8; DATA_PAYLOAD_SIZE + 2];
        buf[0] = self.opcode;
        buf[1] = self.length;
        buf[2..].copy_from_slice(&self.value);
        buf
    }

}
