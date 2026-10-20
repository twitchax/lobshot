//! Sounds: each `lobshot_assets::Sound`'s Ogg Vorbis file, copied unchanged. Vorbis is the one
//! format the client decodes (Bevy's `vorbis` feature), so anything else is refused here rather
//! than failing in a player's browser.

use std::path::Path;

use anyhow::{Context as _, ensure};
use lobshot_assets::{Sound, sound_path};
use lobshot_sim::base::Void;

use super::out::Out;

pub fn build_sounds(sources: &Path, out: &mut Out) -> Void {
    for sound in Sound::ALL {
        let path = sound_path(sound);
        let bytes = std::fs::read(sources.join(&path)).with_context(|| format!("reading assets/{path}"))?;
        ensure!(is_vorbis(&bytes), "assets/{path} isn't Ogg Vorbis, the only format the client decodes");
        out.write(&path, &bytes)?;
    }
    Ok(())
}

/// An Ogg stream whose first packet is a Vorbis identification header.
fn is_vorbis(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS") && bytes.get(..64).is_some_and(|head| head.windows(7).any(|w| w == b"\x01vorbis"))
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::{collections::BTreeSet, io::Cursor};

    use lewton::inside_ogg::OggStreamReader;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;
    use crate::art::sources;

    /// The client decodes with lewton (through rodio), and a file it can't decode panics there, so
    /// every real sound must decode to the end here.
    #[test]
    fn the_real_sounds_decode_to_the_end() {
        for sound in Sound::ALL {
            let path = sound_path(sound);
            let mut reader = OggStreamReader::new(Cursor::new(std::fs::read(sources().join(&path)).unwrap())).unwrap();
            let mut samples = 0;
            while let Some(packet) = reader.read_dec_packet_itl().unwrap_or_else(|e| panic!("{path}: {e}")) {
                samples += packet.len();
            }
            assert!(samples > 0, "{path} is silent");
        }
    }

    #[test]
    fn every_sound_file_is_a_cue_and_credited() {
        let dir = sources().join("sounds");
        let files: BTreeSet<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ogg")))
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        let cues: BTreeSet<String> = Sound::ALL.iter().map(|&sound| sound_path(sound).trim_start_matches("sounds/").to_owned()).collect();
        assert_eq!(files, cues);
        let credits = std::fs::read_to_string(dir.join("CREDITS.md")).unwrap();
        for file in &files {
            assert!(credits.contains(&format!("`{file}`")), "CREDITS.md doesn't say where {file} came from");
        }
    }

    #[test]
    fn copies_each_sound_unchanged_and_refuses_what_isnt_vorbis() {
        let (dir, out_dir) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        std::fs::create_dir_all(dir.path().join("sounds")).unwrap();
        for sound in Sound::ALL {
            std::fs::copy(sources().join(sound_path(sound)), dir.path().join(sound_path(sound))).unwrap();
        }
        let mut out = Out::new(out_dir.path());
        build_sounds(dir.path(), &mut out).unwrap();
        let blast = sound_path(Sound::Blast);
        assert_eq!(std::fs::read(out_dir.path().join(&blast)).unwrap(), std::fs::read(sources().join(&blast)).unwrap());

        std::fs::write(dir.path().join(&blast), b"RIFF....WAVEfmt ").unwrap();
        let error = build_sounds(dir.path(), &mut Out::new(out_dir.path())).unwrap_err();
        assert!(format!("{error:#}").contains("blast.ogg isn't Ogg Vorbis"), "{error:#}");
    }
}
