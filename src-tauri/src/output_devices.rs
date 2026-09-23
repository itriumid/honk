use rodio::cpal::traits::HostTrait;
use rodio::{Device, DeviceTrait};
use serde::Serialize;

/// An output device as the frontend sees it. `id` is cpal's stable identifier and is what gets
/// stored and sent back; `name` is only for display.
#[derive(Debug, Clone, Serialize)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list() -> Result<Vec<OutputDevice>, String> {
    let host = rodio::cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());

    let devices = host
        .output_devices()
        .map_err(|error| format!("could not list output devices: {error}"))?;

    Ok(devices
        .filter_map(|device| {
            let id = device.id().ok()?.to_string();
            let name = device
                .description()
                .map(|description| description.name().to_string())
                .unwrap_or_else(|_| id.clone());
            let is_default = default_id.as_deref() == Some(id.as_str());
            Some(OutputDevice {
                id,
                name,
                is_default,
            })
        })
        .collect())
}

pub fn find(id: &str) -> Result<Device, String> {
    let host = rodio::cpal::default_host();
    host.output_devices()
        .map_err(|error| format!("could not list output devices: {error}"))?
        .find(|device| {
            device
                .id()
                .is_ok_and(|device_id| device_id.to_string() == id)
        })
        .ok_or_else(|| format!("output device {id} is not connected"))
}

pub fn default_device() -> Result<Device, String> {
    rodio::cpal::default_host()
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs real audio hardware, so it's opt-in: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn lists_this_machines_output_devices_and_finds_each_by_id() {
        let devices = list().expect("listing output devices");
        assert!(!devices.is_empty(), "expected at least one output device");
        assert_eq!(devices.iter().filter(|device| device.is_default).count(), 1);
        for device in &devices {
            println!(
                "{} (default: {}) — {}",
                device.name, device.is_default, device.id
            );
            find(&device.id).expect("every listed device should be findable by its id");
        }
    }
}
