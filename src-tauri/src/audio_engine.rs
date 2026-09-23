//! Owns every output stream on one dedicated thread. Streams can't be moved across threads on
//! every platform, so nothing else touches them: callers send commands over a channel and wait
//! for the reply.
//!
//! The engine also tracks what's playing and reports each playback's start and finish through
//! the event callback given to [`AudioEngine::start`]. Every event comes from the engine thread,
//! in order — including finishes detected on the audio thread, which only forwards them here.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use rodio::source::EmptyCallback;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use serde::Serialize;

use crate::output_devices;

/// A sound's encoded file contents, read once and shared by every play of it — including one
/// decode per output device when playing to two at once.
pub type SoundData = Arc<[u8]>;

/// Identifies one play of a sound, so overlapping plays of the same sound are told apart.
pub type PlaybackId = u64;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackEvent {
    Started {
        playback_id: PlaybackId,
        sound_id: i64,
        /// `None` when the file doesn't say, as with some MP3 streams.
        duration_milliseconds: Option<u64>,
    },
    Finished {
        playback_id: PlaybackId,
        sound_id: i64,
    },
}

type Reply<T = ()> = mpsc::Sender<Result<T, String>>;

enum Command {
    Play {
        sound_id: i64,
        sound: SoundData,
        volume: f32,
        reply: Reply<PlaybackId>,
    },
    StopAll {
        reply: Reply,
    },
    SetOutputDevices {
        primary: Option<String>,
        secondary: Option<String>,
        reply: Reply,
    },
    /// Sent from the audio thread when a playback's primary output runs out.
    Finished {
        playback_id: PlaybackId,
    },
}

pub struct AudioEngine {
    commands: mpsc::Sender<Command>,
}

impl AudioEngine {
    pub fn start(events: impl Fn(PlaybackEvent) + Send + 'static) -> Self {
        let (commands, receiver) = mpsc::channel();
        let finished = commands.clone();
        thread::Builder::new()
            .name("audio-engine".into())
            .spawn(move || run(receiver, finished, events))
            .expect("failed to spawn the audio engine thread");
        Self { commands }
    }

    /// Plays `sound` on every configured output device, overlapping anything already playing.
    /// `volume` is linear: 0.0 is silent, 1.0 is the file's own level.
    pub fn play(&self, sound_id: i64, sound: SoundData, volume: f32) -> Result<PlaybackId, String> {
        self.request(|reply| Command::Play {
            sound_id,
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

    fn request<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> Result<T, String> {
        let (reply, response) = mpsc::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| "the audio engine has stopped".to_string())?;
        response
            .recv()
            .map_err(|_| "the audio engine stopped before replying".to_string())?
    }
}

struct State<E> {
    /// Primary first, then the secondary if one is selected. Empty until first needed, so the
    /// app starts without claiming an audio device.
    sinks: Vec<MixerDeviceSink>,
    /// One per sound per device. Dropping a player stops it.
    players: Vec<Player>,
    /// Sound id by playback, for everything started and not yet reported finished.
    playing: HashMap<PlaybackId, i64>,
    next_playback_id: PlaybackId,
    events: E,
}

impl<E: Fn(PlaybackEvent)> State<E> {
    fn new(events: E) -> Self {
        Self {
            sinks: Vec::new(),
            players: Vec::new(),
            playing: HashMap::new(),
            next_playback_id: 1,
            events,
        }
    }

    fn finish(&mut self, playback_id: PlaybackId) {
        // Unknown ids are expected: a stop already reported this playback as finished.
        if let Some(sound_id) = self.playing.remove(&playback_id) {
            (self.events)(PlaybackEvent::Finished {
                playback_id,
                sound_id,
            });
        }
    }

    /// Stops every player. Their end-of-sound callbacks will never fire, so report each
    /// playback as finished here.
    fn stop_everything(&mut self) {
        self.players.clear();
        let mut stopped: Vec<_> = self.playing.keys().copied().collect();
        stopped.sort_unstable();
        for playback_id in stopped {
            self.finish(playback_id);
        }
    }
}

fn run(
    receiver: mpsc::Receiver<Command>,
    finished: mpsc::Sender<Command>,
    events: impl Fn(PlaybackEvent),
) {
    let mut state = State::new(events);
    for command in receiver {
        state.players.retain(|player| !player.empty());
        match command {
            Command::Play {
                sound_id,
                sound,
                volume,
                reply,
            } => {
                let _ = reply.send(play(&mut state, &finished, sound_id, sound, volume));
            }
            Command::StopAll { reply } => {
                state.stop_everything();
                let _ = reply.send(Ok(()));
            }
            Command::SetOutputDevices {
                primary,
                secondary,
                reply,
            } => {
                let result = open_sinks(primary.as_deref(), secondary.as_deref()).map(|sinks| {
                    state.stop_everything();
                    state.sinks = sinks;
                });
                let _ = reply.send(result);
            }
            Command::Finished { playback_id } => state.finish(playback_id),
        }
    }
}

fn play<E: Fn(PlaybackEvent)>(
    state: &mut State<E>,
    finished: &mpsc::Sender<Command>,
    sound_id: i64,
    sound: SoundData,
    volume: f32,
) -> Result<PlaybackId, String> {
    // Decode once up front so a bad file fails before any device is opened.
    let duration = decode(&sound)?.total_duration();

    if state.sinks.is_empty() {
        state.sinks = open_sinks(None, None)?;
    }

    let playback_id = state.next_playback_id;
    state.next_playback_id += 1;

    for (index, sink) in state.sinks.iter().enumerate() {
        let player = Player::connect_new(sink.mixer());
        player.set_volume(volume.clamp(0.0, 1.0));
        player.append(decode(&sound)?);
        // The primary output decides when a playback is over; the secondary plays the same
        // sound on another clock and may end a few milliseconds apart.
        if index == 0 {
            let finished = finished.clone();
            player.append(EmptyCallback::new(Box::new(move || {
                let _ = finished.send(Command::Finished { playback_id });
            })));
        }
        state.players.push(player);
    }

    state.playing.insert(playback_id, sound_id);
    (state.events)(PlaybackEvent::Started {
        playback_id,
        sound_id,
        duration_milliseconds: duration.map(|duration| duration.as_millis() as u64),
    });
    Ok(playback_id)
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
    use std::sync::Mutex;
    use std::time::Duration;

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

    type Recorded = Arc<Mutex<Vec<PlaybackEvent>>>;

    /// Collects every event a `State` emits.
    fn recording_state() -> (State<impl Fn(PlaybackEvent)>, Recorded) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let state = State::new(move |event| sink.lock().unwrap().push(event));
        (state, events)
    }

    #[test]
    fn decodes_a_wav_from_memory_with_its_duration() {
        let sound = wav(&[0; 4410], 44_100);
        let decoder = decode(&sound).expect("valid WAV should decode");
        assert_eq!(decoder.sample_rate().get(), 44_100);
        assert_eq!(decoder.total_duration(), Some(Duration::from_millis(100)));
        assert_eq!(decoder.count(), 4410);
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
    fn a_bad_file_fails_before_any_device_is_opened_or_event_sent() {
        let (mut state, events) = recording_state();
        let (finished, _) = mpsc::channel();
        let sound: SoundData = b"definitely not a sound file".to_vec().into();
        assert!(play(&mut state, &finished, 1, sound, 1.0).is_err());
        assert!(state.sinks.is_empty());
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn a_finish_is_reported_once_and_unknown_ids_are_ignored() {
        let (mut state, events) = recording_state();
        state.playing.insert(7, 42);

        state.finish(7);
        state.finish(7);
        state.finish(99);

        assert_eq!(
            *events.lock().unwrap(),
            [PlaybackEvent::Finished {
                playback_id: 7,
                sound_id: 42
            }]
        );
    }

    #[test]
    fn stopping_reports_every_playback_finished_and_late_callbacks_add_nothing() {
        let (mut state, events) = recording_state();
        state.playing.insert(2, 20);
        state.playing.insert(1, 10);

        state.stop_everything();
        // The audio thread may still deliver a finish for a player that was just dropped.
        state.finish(1);

        assert_eq!(
            *events.lock().unwrap(),
            [
                PlaybackEvent::Finished {
                    playback_id: 1,
                    sound_id: 10
                },
                PlaybackEvent::Finished {
                    playback_id: 2,
                    sound_id: 20
                },
            ]
        );
        assert!(state.playing.is_empty());
    }

    #[test]
    fn events_serialize_for_the_frontend() {
        let started = serde_json::to_value(PlaybackEvent::Started {
            playback_id: 3,
            sound_id: 9,
            duration_milliseconds: None,
        })
        .unwrap();
        assert_eq!(
            started,
            serde_json::json!({
                "kind": "started",
                "playback_id": 3,
                "sound_id": 9,
                "duration_milliseconds": null
            })
        );
    }

    /// Plays a silent tenth of a second on the default device and waits for the finish event.
    /// Needs real audio hardware, so it's opt-in: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn a_real_playback_reports_started_then_finished() {
        let (sender, receiver) = mpsc::channel();
        let engine = AudioEngine::start(move |event| {
            let _ = sender.send(event);
        });
        let playback_id = engine.play(5, wav(&[0; 4410], 44_100), 0.0).unwrap();

        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            PlaybackEvent::Started {
                playback_id,
                sound_id: 5,
                duration_milliseconds: Some(100)
            }
        );
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            PlaybackEvent::Finished {
                playback_id,
                sound_id: 5
            }
        );
    }
}
