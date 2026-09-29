use serde::Serialize;
use std::fs::File;
use std::io::Read;
use zip::ZipArchive;

#[derive(Serialize)]
pub struct ZipEntry {
    pub filename: String,
    pub directory: bool,
}

pub fn read_zip_entries(file_path: &str) -> Result<Vec<ZipEntry>, String> {
    let file =
        File::open(file_path).map_err(|error| format!("Failed to open zip file: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Failed to read zip archive: {error}"))?;
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Failed to read zip entry: {error}"))?;
        entries.push(ZipEntry {
            filename: entry.name().to_string(),
            directory: entry.is_dir(),
        });
    }

    Ok(entries)
}

pub fn read_zip_file(file_path: &str, filename: &str) -> Result<Vec<u8>, String> {
    let file =
        File::open(file_path).map_err(|error| format!("Failed to open zip file: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Failed to read zip archive: {error}"))?;
    let mut entry = archive
        .by_name(filename)
        .map_err(|error| format!("Failed to read zip entry {filename}: {error}"))?;

    if entry.is_dir() {
        return Err(format!("Zip entry {filename} is a directory"));
    }

    let mut content = Vec::new();
    entry
        .read_to_end(&mut content)
        .map_err(|error| format!("Failed to read zip entry {filename}: {error}"))?;
    Ok(content)
}
