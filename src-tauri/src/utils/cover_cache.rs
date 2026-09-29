use image::codecs::jpeg::JpegEncoder;
use natord::compare_ignore_case;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use zip::ZipArchive;

const CACHE_DATABASE_NAME: &str = "comic-cover-cache.sqlite3";
const THUMBNAIL_WIDTH: u32 = 240;
const THUMBNAIL_HEIGHT: u32 = 340;
const THUMBNAIL_QUALITY: u8 = 80;

#[derive(Serialize)]
pub struct ZipCover {
    pub name: String,
    pub preview: Vec<u8>,
    pub preview_mime: String,
    pub count: usize,
}

struct CachedCover {
    preview: Vec<u8>,
    preview_mime: String,
    count: usize,
}

struct ZipEntryInfo {
    filename: String,
    directory: bool,
}

struct FileFingerprint {
    modified_time: i64,
    file_size: i64,
}

pub fn index_zip_covers(cache_dir: &Path, zip_paths: &[String]) -> Result<Vec<ZipCover>, String> {
    fs::create_dir_all(cache_dir)
        .map_err(|error| format!("Failed to create cover cache directory: {error}"))?;

    let database_path = cache_dir.join(CACHE_DATABASE_NAME);
    let mut connection = Connection::open(&database_path)
        .map_err(|error| format!("Failed to open cover cache database: {error}"))?;
    initialize_database(&connection)?;

    let transaction = connection
        .transaction()
        .map_err(|error| format!("Failed to start cover cache transaction: {error}"))?;
    let mut covers = Vec::with_capacity(zip_paths.len());

    for zip_path in zip_paths {
        let fingerprint = read_file_fingerprint(zip_path)?;
        let cached_cover = read_cached_cover(&transaction, zip_path, &fingerprint)?;
        let cover = match cached_cover {
            Some(cover) => cover,
            None => {
                let cover = index_zip_cover(zip_path)?;
                store_cached_cover(&transaction, zip_path, &fingerprint, &cover)?;
                cover
            }
        };

        covers.push(ZipCover {
            name: display_name(zip_path),
            preview: cover.preview,
            preview_mime: cover.preview_mime,
            count: cover.count,
        });
    }

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit cover cache: {error}"))?;

    Ok(covers)
}

fn initialize_database(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS zip_cover_cache (
                 zip_path TEXT PRIMARY KEY NOT NULL,
                 modified_time INTEGER NOT NULL,
                 file_size INTEGER NOT NULL,
                 page_count INTEGER NOT NULL,
                 preview BLOB NOT NULL,
                 preview_mime TEXT NOT NULL
             );",
        )
        .map_err(|error| format!("Failed to initialize cover cache database: {error}"))
}

fn read_cached_cover(
    transaction: &Transaction<'_>,
    zip_path: &str,
    fingerprint: &FileFingerprint,
) -> Result<Option<CachedCover>, String> {
    transaction
        .query_row(
            "SELECT page_count, preview, preview_mime
             FROM zip_cover_cache
             WHERE zip_path = ?1 AND modified_time = ?2 AND file_size = ?3",
            params![zip_path, fingerprint.modified_time, fingerprint.file_size],
            |row| {
                let page_count = row.get::<_, i64>(0)?;
                let count = usize::try_from(page_count).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;

                Ok(CachedCover {
                    count,
                    preview: row.get(1)?,
                    preview_mime: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Failed to read cover cache entry: {error}"))
}

fn store_cached_cover(
    transaction: &Transaction<'_>,
    zip_path: &str,
    fingerprint: &FileFingerprint,
    cover: &CachedCover,
) -> Result<(), String> {
    let page_count = i64::try_from(cover.count)
        .map_err(|error| format!("ZIP page count is too large to cache: {error}"))?;

    transaction
        .execute(
            "INSERT INTO zip_cover_cache
                 (zip_path, modified_time, file_size, page_count, preview, preview_mime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(zip_path) DO UPDATE SET
                 modified_time = excluded.modified_time,
                 file_size = excluded.file_size,
                 page_count = excluded.page_count,
                 preview = excluded.preview,
                 preview_mime = excluded.preview_mime",
            params![
                zip_path,
                fingerprint.modified_time,
                fingerprint.file_size,
                page_count,
                &cover.preview,
                &cover.preview_mime,
            ],
        )
        .map_err(|error| format!("Failed to write cover cache entry: {error}"))?;

    Ok(())
}

fn read_file_fingerprint(zip_path: &str) -> Result<FileFingerprint, String> {
    let metadata = fs::metadata(zip_path)
        .map_err(|error| format!("Failed to read ZIP metadata for {zip_path}: {error}"))?;
    let modified_time = metadata
        .modified()
        .map_err(|error| format!("Failed to read ZIP modification time for {zip_path}: {error}"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Invalid ZIP modification time for {zip_path}: {error}"))?
        .as_millis();
    let modified_time = i64::try_from(modified_time)
        .map_err(|error| format!("ZIP modification time is too large for {zip_path}: {error}"))?;
    let file_size = i64::try_from(metadata.len())
        .map_err(|error| format!("ZIP file size is too large for {zip_path}: {error}"))?;

    Ok(FileFingerprint {
        modified_time,
        file_size,
    })
}

fn index_zip_cover(zip_path: &str) -> Result<CachedCover, String> {
    let file = File::open(zip_path)
        .map_err(|error| format!("Failed to open ZIP file {zip_path}: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Failed to read ZIP archive {zip_path}: {error}"))?;
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("Failed to read ZIP entry in {zip_path}: {error}"))?;
        entries.push(ZipEntryInfo {
            filename: entry.name().to_string(),
            directory: entry.is_dir(),
        });
    }

    entries.sort_by(|left, right| compare_ignore_case(&left.filename, &right.filename));

    let cover_entry = entries.iter().find(|entry| {
        !entry.directory
            && entry
                .filename
                .rsplit_once('.')
                .map(|(_, extension)| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp"
                    )
                })
                .unwrap_or(false)
    });

    let (preview, preview_mime) = match cover_entry {
        Some(entry) => {
            let mut image_entry = archive
                .by_name(&entry.filename)
                .map_err(|error| format!("Failed to read cover entry in {zip_path}: {error}"))?;
            let mut image_data = Vec::new();
            image_entry
                .read_to_end(&mut image_data)
                .map_err(|error| format!("Failed to read cover data in {zip_path}: {error}"))?;
            create_thumbnail(&image_data, &entry.filename)
        }
        None => (Vec::new(), String::new()),
    };

    Ok(CachedCover {
        preview,
        preview_mime,
        count: entries.len(),
    })
}

fn create_thumbnail(image_data: &[u8], filename: &str) -> (Vec<u8>, String) {
    let original_mime = mime_for_filename(filename);

    if original_mime == "image/svg+xml" {
        return (image_data.to_vec(), original_mime.to_string());
    }

    let thumbnail = match image::load_from_memory(image_data) {
        Ok(image) => image.thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT),
        Err(_) => return (image_data.to_vec(), original_mime.to_string()),
    };

    let mut preview = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut preview, THUMBNAIL_QUALITY);
    if thumbnail.write_with_encoder(encoder).is_ok() {
        return (preview, "image/jpeg".to_string());
    }

    (image_data.to_vec(), original_mime.to_string())
}

fn mime_for_filename(filename: &str) -> &str {
    match Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn display_name(zip_path: &str) -> String {
    PathBuf::from(zip_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| zip_path.to_string())
}
