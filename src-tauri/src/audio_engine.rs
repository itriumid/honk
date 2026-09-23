//! Owns every output stream on one dedicated thread. Streams can't be moved across threads on
//! every platform, so nothing else touches them: callers send commands over a channel and wait
//! for the reply.

use std::io::Cursor;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::output_devices;

/// A sound's encoded file contents, read once and shared by every play of it — including one
/// decode per output device when playing to two at once.
pub type SoundData = Arc<[u8]>;

type Reply = mpsc::Sender<Result<(), String>>;

enum Command {
    Play {
        sound: SoundData,
        volume: f32,
        reply: Reply,
    },
    StopAll {
        reply: Reply,
    },
    SetOutputDevices {
        primary: Option<String>,
        secondary: Option<String>,
        reply: Reply,
    },
}

pub struct AudioEngine {
    commands: mpsc::Sender<Command>,
}

impl AudioEngine {
    pub fn start() -> Self {
        let (commands, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || run(receiver))
            .expect("failed to spawn the audio engine thread");
        Self { commands }
    }

    /// Plays `sound` on every configured output device, overlapping anything already playing.
    /// `volume` is linear: 0.0 is silent, 1.0 is the file's own level.
    pub fn play(&self, sound: SoundData, volume: f32) -> Result<(), String> {
        self.request(|reply| Command::Play {
            sound,
            volume,
            reply,
        })
    }

    pub fn stop_all(&self) -> Result<(), String> {
        self.request(|reply| Command::StopAll { reply })
    }

    /// Selects output devices by id; `None` for the primary means the system default. Takes
    /// effect immediately and stops whatever is playing.
    pub fn set_output_devices(
        &self,
        primary: Option<String>,
        secondary: Option<String>,
    ) -> Result<(), String> {
        self.request(|reply| Command::SetOutputDevices {
            primary,
            secondary,
            reply,
        })
    }

    fn request(&self, command: impl FnOnce(Reply) -> Command) -> Result<(), String> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| "the audio engine has stopped".to_string())?;
        response
            .recv()
            .map_err(|_| "the audio engine stopped before replying".to_string())?
    }
}

#[derive(Default)]
struct State {
    /// Primary first, then the secondary if one is selected. Empty until first needed, so the
    /// app starts without claiming an audio device.
    sinks: Vec<MixerDeviceSink>,
    /// One per sound per device. Dropping a player stops it.
    players: Vec<Player>,
}

fn run(receiver: mpsc::Receiver<Command>) {
    let mut state = State::default();
    for command in receiver {
        state.players.retain(|player| !player.empty());
        match command {
            Command::Play {
                sound,
                volume,
                reply,
            } => {
                let _ = reply.send(play(&mut state, sound, volume));
            }
            Command::StopAll { reply } => {
                state.players.clear();
                let _ = reply.send(Ok(()));
            }
            Command::SetOutputDevices {
                primary,
                secondary,
                reply,
            } => {
                let result = open_sinks(primary.as_deref(), secondary.as_deref()).map(|sinks| {
                    state.players.clear();
                    state.sinks = sinks;
                });
                let _ = reply.send(result);
            }
        }
    }
}

fn play(state: &mut State, sound: SoundData, volume: f32) -> Result<(), String> {
    // Decode once up front so a bad file fails before any device is opened.
    decode(&sound)?;

    if state.sinks.is_empty() {
        state.sinks = open_sinks(None, None)?;
    }

    for sink in &state.sinks {
        let player = Player::connect_new(sink.mixer());
        player.set_volume(volume.clamp(0.0, 1.0));
        player.append(decode(&sound)?);
        state.players.push(player);
    }
    Ok(())
}

pub fn decode(sound: &SoundData) -> Result<Decoder<Cursor<SoundData>>, String> {
    Decoder::try_from(Cursor::new(Arc::clone(sound)))
        .map_err(|error| format!("could not decode the sound: {error}"))
}

fn open_sinks(
    primary: Option<&str>,
    secondary: Option<&str>,
) -> Result<Vec<MixerDeviceSink>, String> {
    let mut sinks = vec![open_sink(primary)?];
    if let Some(secondary) = secondary {
        if Some(secondary) != primary {
            sinks.push(open_sink(Some(secondary))?);
        }
    }
    Ok(sinks)
}

fn open_sink(device_id: Option<&str>) -> Result<MixerDeviceSink, String> {
    let device = match device_id {
        Some(id) => output_devices::find(id)?,
        None => output_devices::default_device()?,
    };
    let mut sink = DeviceSinkBuilder::from_device(device)
        .and_then(|builder| builder.open_sink_or_fallback())
        .map_err(|error| format!("could not open the output device: {error}"))?;
    // Replacing sinks on a device change is expected, not worth a line on stderr each time.
    sink.log_on_drop(false);
    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    /// A minimal 16-bit mono PCM WAV file, built by hand so the tests need no fixture files.
    fn wav(samples: &[i16], sample_rate: u32) -> SoundData {
        let data_length = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes.into()
    }

    #[test]
    fn decodes_a_wav_from_memory() {
        let sound = wav(&[0, 1000, -1000, 0], 44_100);
        let decoder = decode(&sound).expect("valid WAV should decode");
        assert_eq!(decoder.sample_rate().get(), 44_100);
        assert_eq!(decoder.count(), 4);
    }

    #[test]
    fn decodes_the_same_sound_twice_for_dual_output() {
        let sound = wav(&[0, 1000, -1000, 0], 44_100);
        assert_eq!(decode(&sound).unwrap().count(), 4);
        assert_eq!(decode(&sound).unwrap().count(), 4);
    }

    #[test]
    fn rejects_data_that_is_not_audio() {
        let sound: SoundData = b"definitely not a sound file".to_vec().into();
        assert!(decode(&sound).is_err());
    }

    #[test]
    fn a_bad_file_fails_before_any_device_is_opened() {
        let mut state = State::default();
        let sound: SoundData = b"definitely not a sound file".to_vec().into();
        assert!(play(&mut state, sound, 1.0).is_err());
        assert!(state.sinks.is_empty());
    }
}
