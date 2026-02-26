use cpal::platform::{Device, Host};
use cpal::traits::HostTrait;

use pulsectl::controllers::{AppControl, DeviceControl, SourceController};

use crate::audio_controllers::audio_backend::AudioBackend;
use crate::core::thread_messages::DeviceListItem;

use log::debug;

pub struct PulseBackend {
    handler: SourceController,
}

impl PulseBackend {
    pub fn try_init() -> Option<Self> {
        if let Ok(mut handler) = SourceController::create() {
            if let Ok(_) = handler.get_server_info() {
                if let Ok(_) = handler.list_devices() {
                    return Some(Self { handler });
                }
            }
        }
        None
    }

    fn get_app_idx(&mut self) -> Option<u32> {
        // Get SongRec's source-output index

        let applications = self.handler.list_applications().unwrap();

        let criteria: Vec<String> = vec![
            format!("process.id = \"{}\"", std::process::id()),
            "alsa plug-in [songrec]".to_string(),
            "songrec".to_string(),
            format!("{}", std::process::id()),
        ];

        for criterion in criteria {
            for app in applications.clone() {
                if app
                    .proplist
                    .to_string()
                    .unwrap()
                    .to_lowercase()
                    .contains(&criterion)
                {
                    return Some(app.index);
                }
            }
        }
        None
    }
}

impl AudioBackend for PulseBackend {
    fn list_devices(&mut self, _host: &Host) -> Vec<DeviceListItem> {
        let mut device_names: Vec<DeviceListItem> = vec![];
        let mut monitor_device_names: Vec<DeviceListItem> = vec![];

        if let Ok(info) = self.handler.get_server_info() {
            if let Ok(devices) = self.handler.list_devices() {
                for dev in devices {
                    if let Some(desc) = &dev.description {
                        if let Some(name) = &dev.name {
                            if &dev.name == &info.default_source_name {
                                device_names.insert(
                                    0,
                                    DeviceListItem {
                                        inner_name: name.to_string(),
                                        display_name: desc.to_string(),
                                        is_monitor: dev.monitor != None,
                                    },
                                );
                            } else if dev.monitor != None {
                                monitor_device_names.push(DeviceListItem {
                                    inner_name: name.to_string(),
                                    display_name: desc.to_string(),
                                    is_monitor: true,
                                });
                            } else {
                                device_names.push(DeviceListItem {
                                    inner_name: name.to_string(),
                                    display_name: desc.to_string(),
                                    is_monitor: false,
                                });
                            }
                        }
                    }
                }
            }
        }

        device_names.extend(monitor_device_names);
        device_names
    }

    fn set_device(&mut self, host: &Host, inner_name: &str) -> Device {
        if let Ok(devices) = self.handler.list_devices() {
            if let Some(app_idx) = self.get_app_idx() {
                for dev in devices {
                    debug!(
                        "Comparing libpulse device names: {:?} / {:?}",
                        dev.name, inner_name
                    );
                    if Some(inner_name) == dev.name.as_deref() {
                        debug!("Selected libpulse device found: {:?}", dev);

                        self.handler.move_app_by_name(app_idx, inner_name).unwrap();
                        break;
                    }
                }
            }
        }

        host.default_input_device().unwrap()
    }
}
