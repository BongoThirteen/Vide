
use std::{fs::File, io::Write};

use ac_ffmpeg::{codec::{video::{VideoEncoder, self, VideoFrameMut, PixelFormat}, Encoder}, time::{TimeBase, Timestamp}, format::{muxer::{Muxer, OutputFormat}, io::IO}};

use ffimage::iter::{BytesExt, PixelsExt};
use ffimage::Pixel;

use ffimage_yuv::yuv::Yuv;

use vide_lib::io::Export;

struct Rgba<T, const R: usize = 0, const G: usize = 1, const B: usize = 2, const A: usize = 3>(pub [T; 4]);

impl<T, const R: usize, const G: usize, const B: usize, const A: usize> Pixel for Rgba<T, R, G, B, A> {
    const CHANNELS: u8 = 4;
}

impl<T, const R: usize, const G: usize, const B: usize, const A: usize> From<[T; 4]> for Rgba<T, R, G, B, A> {
    fn from(value: [T; 4]) -> Self {
        Self(value)
    }
}

impl<
    const Y: usize,
    const U: usize,
    const V: usize,
    const R: usize,
    const G: usize,
    const B: usize,
    const A: usize,
> Into<Yuv<u8, Y, U, V>> for Rgba<u8, R, G, B, A> {
    fn into(self) -> Yuv<u8, Y, U, V> {
        let r = self.0[R] as i32;
        let g = self.0[G] as i32;
        let b = self.0[B] as i32;

        let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
        let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
        let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;

        let mut yuv = Yuv::<u8, Y, U, V>::default();
        yuv[Y] = y as u8;
        yuv[U] = u as u8;
        yuv[V] = v as u8;
        yuv
    }
}

fn open_output(path: &str, elementary_streams: &[ac_ffmpeg::codec::CodecParameters]) -> Result<Muxer<File>, ac_ffmpeg::Error> {
    let output_format = OutputFormat::guess_from_file_name(path)
        .ok_or_else(|| ac_ffmpeg::Error::new(format!("unable to guess output format for file: {}", path)))?;

    let output = File::create(path)
        .map_err(|err| ac_ffmpeg::Error::new(format!("unable to create output file {}: {}", path, err)))?;

    let io = IO::from_seekable_write_stream(output);

    let mut muxer_builder = Muxer::builder();

    for codec_parameters in elementary_streams {
        muxer_builder.add_stream(codec_parameters)?;
    }

    muxer_builder.build(io, output_format)
}

pub struct FFmpegExporter {
    output: String,

    container: String,
    video_coding: String,
    pixel_formatting: String,
    audio_coding: Option<String>,

    encoder: Option<VideoEncoder>,
    muxer: Option<Muxer<File>>,

    current_timestamp: i64,
    ms_per_frame: i64,
    pixel_format: Option<PixelFormat>,
    resolution: (usize, usize),
}

impl FFmpegExporter { // TODO: Support multiple encoders and stuff
    pub fn new(
        output: impl ToString,
        container: impl ToString,
        video_coding: impl ToString,
        pixel_format: impl ToString,
        audio_coding: Option<String>
    ) -> Self {
        Self {
            output: output.to_string(),

            container: container.to_string(),
            video_coding: video_coding.to_string(),
            pixel_formatting: pixel_format.to_string(),
            audio_coding: audio_coding,

            encoder: None,
            muxer: None,

            current_timestamp: 0,
            ms_per_frame: 0,
            pixel_format: None,
            resolution: (1920, 1080),
        }
    }
}

impl Export for FFmpegExporter {
    fn begin(&mut self, settings: vide_lib::api::video::VideoSettings) {
        let time_base = TimeBase::new(1, 1_000_000);
        let pixel_format = video::frame::get_pixel_format(&self.pixel_formatting);
        
        let encoder = VideoEncoder::builder(&self.video_coding)
            .unwrap()
            .pixel_format(pixel_format)
            .width(settings.resolution.0 as usize)
            .height(settings.resolution.1 as usize)
            .time_base(time_base)
            .build()
            .unwrap();
        
        let codec_parameters = encoder.codec_parameters().into();
        let muxer = open_output(self.output.as_str(), &[codec_parameters]).unwrap();

        self.encoder = Some(encoder);
        self.muxer = Some(muxer);
        self.ms_per_frame = ((1.0 / settings.fps) * 1000000.0) as i64;
        self.pixel_format = Some(pixel_format);
        self.resolution = (settings.resolution.0 as usize, settings.resolution.1 as usize);
    }

    fn push_frame(&mut self, _keyframe: bool, frame: &[u8]) {
        let timestamp = Timestamp::from_micros(self.current_timestamp);
        let encoder = self.encoder.as_mut().unwrap();
        let muxer = self.muxer.as_mut().unwrap();

        {
            // Allocate frame
            let mut new_frame = VideoFrameMut::black(self.pixel_format.unwrap(), self.resolution.0, self.resolution.1);
            // Remove 4th component of each pixel, convert to YUV and unpack
            let plane_size = self.resolution.0 * self.resolution.1;
            let (mut y, mut u, mut v) = (vec![0; plane_size], vec![0; plane_size], vec![0; plane_size]);
            frame
                .iter()
                .copied()
                .pixels::<Rgba<u8>>()
                .map(<Rgba<u8> as Into<Yuv<u8>>>::into)
                .bytes()
                .enumerate()
                .for_each(|(i, yuv)| {
                    y[i] = yuv[0];
                    u[i] = yuv[1];
                    v[i] = yuv[2];
                });
            // Copy planes to frame
            let mut yuv = new_frame.planes_mut();
            yuv[0].data_mut().write_all(&y).unwrap();
            yuv[1].data_mut().write_all(&u).unwrap();
            yuv[2].data_mut().write_all(&v).unwrap();
            // Add to encoder queue
            encoder.push(new_frame.with_pts(timestamp).freeze()).unwrap();
        }

        // Await encoder and add to muxer queue
        while let Some(packet) = encoder.take().unwrap() {
            muxer.push(packet.with_stream_index(0)).unwrap();
        }

        self.current_timestamp += self.ms_per_frame;
    }

    fn end(mut self) {
        let encoder = self.encoder.as_mut().unwrap();
        let muxer = self.muxer.as_mut().unwrap();

        encoder.flush().unwrap();
        while let Some(packet) = encoder.take().unwrap() {
            muxer.push(packet.with_stream_index(0)).unwrap();
        }
        muxer.flush().unwrap();
    }
}