
use vide_lib::io::Import;

pub struct FFmpegImporter {

}

impl Import for FFmpegImporter {
    fn supported_import_extensions() -> Vec<String> {
        todo!()
    }
}