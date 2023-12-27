
use std::fs::File;

use vide_lib::io::Import;

use ac_ffmpeg::format::{demuxer::Demuxer, io::IO};

fn open_input(path: &str) -> Result<Demuxer<File>, ac_ffmpeg::Error> {
    let input = File::create(path)
        .map_err(|err| ac_ffmpeg::Error::new(format!("unable to create output file {}: {}", path, err)))?;

    let io = IO::from_seekable_read_stream(input);

    Demuxer::builder().build(io)
}

pub struct FFmpegImporter {
        
}

impl Import for FFmpegImporter {
    fn supported_import_extensions() -> Vec<String> {
        todo!()
    }
}