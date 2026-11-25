//! Parallel-compressed ZIP container for `.trazo` and `.trazot` files (T-06).
//!
//! Stores `project.json` deflated in independent 1 MiB pieces in parallel across
//! CPU cores (`rayon` + `flate2` with sync-flush boundaries, pigz-style), plus
//! `pieces.json` enabling parallel decompression and an optional `thumbnail.png`
//! preview readable straight from the ZIP central directory without inflating
//! the model. Plain UTF-8 JSON files remain transparently readable.

use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use flate2::{Compress, Compression, Crc, Decompress, FlushCompress, FlushDecompress, Status};
use image::{ImageFormat, Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::domain::model::Project;

pub const PROJECT_ENTRY: &str = "project.json";
const PIECES_ENTRY: &str = "pieces.json";
pub const THUMBNAIL_ENTRY: &str = "thumbnail.png";

/// Chunk size (1 MiB) for parallel deflate/inflate.
const PIECE_BYTES: usize = 1 << 20;
const MAX_PIECES: usize = 1 << 16;
const MAX_ENTRIES: usize = 64;
const MAX_DIRECTORY_BYTES: usize = MAX_ENTRIES * (46 + 3 * 65_535);
const MAX_THUMBNAIL_BYTES: u64 = 4 << 20;
const LOCAL_SIG: &[u8; 4] = b"PK\x03\x04";
const DAMAGED: &str = "O arquivo .trazo está corrompido (ZIP inválido).";

/// Returns `true` if `bytes` start with a standard ZIP local file header.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(LOCAL_SIG)
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pieces {
    packed: Vec<u64>,
    raw: Vec<u64>,
}

/// Packs `json` (and an optional PNG `thumbnail`) into a standard ZIP container
/// using parallel 1 MiB deflate chunks across CPU cores.
pub fn pack_project(json: &str, thumbnail: Option<&[u8]>, level: u32) -> Result<Vec<u8>, String> {
    let data = json.as_bytes();
    if data.len() as u64 >= u64::from(u32::MAX) {
        return Err("O projeto excede o limite de 4 GiB do contêiner .trazo.".to_owned());
    }
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![data]
    } else {
        data.chunks(PIECE_BYTES).collect()
    };
    let last = chunks.len().saturating_sub(1);
    let deflated: Vec<(Vec<u8>, Crc)> = chunks
        .par_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut crc = Crc::new();
            crc.update(chunk);
            deflate_chunk(chunk, level, i == last).map(|buf| (buf, crc))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut total_crc = Crc::new();
    for (_, part_crc) in &deflated {
        total_crc.combine(part_crc);
    }
    let pieces = Pieces {
        packed: deflated.iter().map(|(d, _)| d.len() as u64).collect(),
        raw: chunks.iter().map(|c| c.len() as u64).collect(),
    };
    let packed: Vec<u8> = deflated.into_iter().flat_map(|(d, _)| d).collect();
    let index_bytes = serde_json::to_vec(&pieces).map_err(|e| e.to_string())?;
    let mut index_crc = Crc::new();
    index_crc.update(&index_bytes);

    let mut zip = ZipWriter::default();
    zip.add(
        PROJECT_ENTRY,
        8,
        &packed,
        total_crc.sum(),
        data.len() as u64,
    )?;
    zip.add(
        PIECES_ENTRY,
        0,
        &index_bytes,
        index_crc.sum(),
        index_bytes.len() as u64,
    )?;
    if let Some(png) = thumbnail
        && !png.is_empty()
    {
        let mut thumb_crc = Crc::new();
        thumb_crc.update(png);
        zip.add(THUMBNAIL_ENTRY, 0, png, thumb_crc.sum(), png.len() as u64)?;
    }
    zip.finish()
}

fn deflate_chunk(chunk: &[u8], level: u32, is_last: bool) -> Result<Vec<u8>, String> {
    let mut compressor = Compress::new(Compression::new(level.min(9)), false);
    let mut out = Vec::with_capacity(chunk.len() / 4 + 64);
    let flush = if is_last {
        FlushCompress::Finish
    } else {
        FlushCompress::Sync
    };
    loop {
        let before = compressor.total_in() as usize;
        out.reserve(chunk.len() / 8 + 1024);
        let status = compressor
            .compress_vec(&chunk[before.min(chunk.len())..], &mut out, flush)
            .map_err(|e| format!("Falha ao comprimir o projeto: {e}"))?;
        let done = compressor.total_in() as usize >= chunk.len();
        match status {
            Status::StreamEnd => return Ok(out),
            Status::Ok | Status::BufError if done && !is_last && out.len() < out.capacity() => {
                return Ok(out);
            }
            Status::Ok | Status::BufError => {}
        }
    }
}

/// Reports the declared uncompressed size of `project.json` inside a ZIP container
/// (used for pre-flight `MemoryBudget` checks before inflating).
pub fn inflated_size(bytes: &[u8]) -> Option<u64> {
    if !is_zip(bytes) {
        return None;
    }
    parse_entries(bytes)
        .ok()?
        .into_iter()
        .find(|e| e.name == PROJECT_ENTRY)
        .map(|e| e.size)
}

/// Unpacks `project.json` from a `.trazo`/`.trazot` ZIP container (bounded to
/// `max_inflated_bytes`) or returns plain UTF-8 JSON transparently for legacy files.
pub fn unpack_project(bytes: &[u8], max_inflated_bytes: u64) -> Result<String, String> {
    if !is_zip(bytes) {
        if bytes.len() as u64 > max_inflated_bytes {
            return Err(format!(
                "O arquivo excede o limite de {} bytes para leitura.",
                max_inflated_bytes
            ));
        }
        return String::from_utf8(bytes.to_vec())
            .map_err(|e| format!("O arquivo não está em UTF-8 válido: {e}"));
    }

    let entries = parse_entries(bytes)?;
    let Some(entry) = entries.iter().find(|e| e.name == PROJECT_ENTRY) else {
        return Err("Arquivo ZIP sem `project.json`: não é um projeto Trazo.".to_owned());
    };
    if entry.size > max_inflated_bytes {
        return Err(format!(
            "O projeto descomprimido ocupa {} bytes, acima do limite de segurança ({} bytes).",
            entry.size, max_inflated_bytes
        ));
    }

    let data = entry.data(bytes)?;
    let raw = match entry.method {
        0 => data.to_vec(),
        8 => {
            let pieces = entries
                .iter()
                .find(|e| e.name == PIECES_ENTRY && e.method == 0 && e.size <= (4 << 20))
                .and_then(|e| e.data(bytes).ok())
                .and_then(|slice| {
                    let parsed: Pieces = serde_json::from_slice(slice).ok()?;
                    parsed.fits(data.len() as u64, entry.size).then_some(parsed)
                });
            match pieces {
                Some(p) => inflate_pieces(data, &p)?,
                None => inflate_whole(data, entry.size.min(max_inflated_bytes))?,
            }
        }
        method => {
            return Err(format!(
                "Método de compressão ZIP não suportado ({method})."
            ));
        }
    };

    if raw.len() as u64 != entry.size {
        return Err(
            "Tamanho descomprimido diverge do cabeçalho ZIP: arquivo danificado.".to_owned(),
        );
    }
    let mut crc = Crc::new();
    crc.update(&raw);
    if crc.sum() != entry.crc {
        return Err("Verificação de integridade (CRC32) falhou: arquivo danificado.".to_owned());
    }
    String::from_utf8(raw).map_err(|e| format!("Conteúdo JSON inválido em UTF-8: {e}"))
}

/// Reads the embedded `thumbnail.png` from a `.trazo`/`.trazot` file without
/// decompressing `project.json`. Returns `None` for plain JSON or missing thumbnails.
pub fn read_thumbnail(path: &Path) -> Option<Vec<u8>> {
    read_thumbnail_from_reader(&mut fs::File::open(path).ok()?)
}

#[cfg(test)]
pub fn read_thumbnail_from_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    read_thumbnail_from_reader(&mut Cursor::new(bytes))
}

fn read_thumbnail_from_reader(reader: &mut (impl Read + Seek)) -> Option<Vec<u8>> {
    let entries = parse_entries_reader(reader).ok()?;
    let entry = entries.into_iter().find(|e| e.name == THUMBNAIL_ENTRY)?;
    // Cap thumbnail to 4 MiB to protect Start Page rendering.
    if entry.size == 0
        || entry.size > MAX_THUMBNAIL_BYTES
        || entry.packed > MAX_THUMBNAIL_BYTES + 65_536
    {
        return None;
    }
    reader.seek(SeekFrom::Start(entry.local_offset)).ok()?;
    let mut header = [0; 30];
    reader.read_exact(&mut header).ok()?;
    let start = entry.data_start(&header).ok()?;
    reader.seek(SeekFrom::Start(start)).ok()?;
    let mut data = vec![0; usize::try_from(entry.packed).ok()?];
    reader.read_exact(&mut data).ok()?;
    let raw = match entry.method {
        0 => data,
        8 => inflate_whole(&data, entry.size).ok()?,
        _ => return None,
    };
    let mut crc = Crc::new();
    crc.update(&raw);
    if raw.len() as u64 != entry.size || crc.sum() != entry.crc {
        return None;
    }
    // Compressed byte size does not bound pixel allocation. Check dimensions
    // before the Start Page decoder allocates a texture for an external file.
    let (width, height) = image::ImageReader::with_format(Cursor::new(&raw), ImageFormat::Png)
        .into_dimensions()
        .ok()?;
    (width > 0 && height > 0 && width <= 1024 && height <= 1024).then_some(raw)
}

impl Pieces {
    fn fits(&self, packed: u64, raw: u64) -> bool {
        let sum = |v: &[u64]| v.iter().try_fold(0u64, |acc, &x| acc.checked_add(x));
        self.packed.len() == self.raw.len()
            && !self.packed.is_empty()
            && self.packed.len() <= MAX_PIECES
            && sum(&self.packed) == Some(packed)
            && sum(&self.raw) == Some(raw)
            && self.raw.iter().all(|&r| r <= PIECE_BYTES as u64)
    }
}

fn inflate_whole(data: &[u8], max_bytes: u64) -> Result<Vec<u8>, String> {
    let likely = (data.len() as u64)
        .saturating_mul(16)
        .min(max_bytes)
        .min(1 << 28);
    let mut out = Vec::with_capacity(usize::try_from(likely).unwrap_or(0));
    flate2::read::DeflateDecoder::new(data)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|e| format!("Não foi possível descomprimir o arquivo: {e}"))?;
    Ok(out)
}

fn inflate_pieces(data: &[u8], pieces: &Pieces) -> Result<Vec<u8>, String> {
    let mut offset = 0usize;
    let mut slices = Vec::with_capacity(pieces.packed.len());
    for (&packed, &raw) in pieces.packed.iter().zip(&pieces.raw) {
        let end = offset.saturating_add(packed as usize);
        let slice = data.get(offset..end).ok_or_else(|| DAMAGED.to_owned())?;
        slices.push((slice, raw));
        offset = end;
    }
    let parts: Vec<Vec<u8>> = slices
        .par_iter()
        .map(|&(slice, raw_len)| {
            let mut out = Vec::with_capacity(raw_len as usize);
            let mut decompressor = Decompress::new(false);
            loop {
                let (before_in, before_out) = (decompressor.total_in() as usize, out.len());
                out.reserve((raw_len as usize).saturating_sub(out.len()).max(64));
                let status = decompressor
                    .decompress_vec(
                        &slice[before_in.min(slice.len())..],
                        &mut out,
                        FlushDecompress::Sync,
                    )
                    .map_err(|e| format!("Falha ao descomprimir bloco: {e}"))?;
                if out.len() as u64 > raw_len {
                    return Err("Bloco descomprimido excede o tamanho declarado.".to_owned());
                }
                if status == Status::StreamEnd
                    || (decompressor.total_in() as usize >= slice.len()
                        && out.len() as u64 == raw_len)
                {
                    break;
                }
                if decompressor.total_in() as usize == before_in && out.len() == before_out {
                    return Err("Bloco comprimido terminou prematuramente.".to_owned());
                }
            }
            Ok(out)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let total_raw = pieces.raw.iter().sum::<u64>() as usize;
    let mut combined = Vec::with_capacity(total_raw);
    for part in parts {
        combined.extend_from_slice(&part);
    }
    Ok(combined)
}

struct ZipEntry {
    name: String,
    method: u16,
    crc: u32,
    packed: u64,
    size: u64,
    local_offset: u64,
}

impl ZipEntry {
    fn data_start(&self, header: &[u8]) -> Result<u64, String> {
        if header.get(..4) != Some(LOCAL_SIG.as_slice())
            || (self.method == 0 && self.packed != self.size)
        {
            return Err(DAMAGED.to_owned());
        }
        let name_len = read_u16(header, 26).ok_or_else(|| DAMAGED.to_owned())?;
        let extra_len = read_u16(header, 28).ok_or_else(|| DAMAGED.to_owned())?;
        self.local_offset
            .checked_add(30 + name_len as u64 + extra_len as u64)
            .ok_or_else(|| DAMAGED.to_owned())
    }

    fn data<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], String> {
        let local = usize::try_from(self.local_offset).map_err(|_| DAMAGED.to_owned())?;
        let header = bytes
            .get(local..local.saturating_add(30))
            .ok_or_else(|| DAMAGED.to_owned())?;
        let start = usize::try_from(self.data_start(header)?).map_err(|_| DAMAGED.to_owned())?;
        let len = usize::try_from(self.packed).map_err(|_| DAMAGED.to_owned())?;
        bytes
            .get(start..start.saturating_add(len))
            .ok_or_else(|| DAMAGED.to_owned())
    }
}

fn read_u16(b: &[u8], at: usize) -> Option<usize> {
    let raw: [u8; 2] = b.get(at..at.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(raw) as usize)
}

fn read_u32(b: &[u8], at: usize) -> Option<u32> {
    let raw: [u8; 4] = b.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn parse_entries(b: &[u8]) -> Result<Vec<ZipEntry>, String> {
    parse_entries_reader(&mut Cursor::new(b))
}

/// Read only the ZIP tail and bounded central directory, even for multi-GB projects.
fn parse_entries_reader(reader: &mut (impl Read + Seek)) -> Result<Vec<ZipEntry>, String> {
    let damaged = || DAMAGED.to_owned();
    let len = reader.seek(SeekFrom::End(0)).map_err(|_| damaged())?;
    let tail_len = len.min(22 + 65_535) as usize;
    reader
        .seek(SeekFrom::End(-(tail_len as i64)))
        .map_err(|_| damaged())?;
    let mut tail = vec![0; tail_len];
    reader.read_exact(&mut tail).map_err(|_| damaged())?;
    let end = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            tail.get(i..i + 4) == Some(b"PK\x05\x06".as_slice())
                && read_u16(&tail, i + 20).is_some_and(|comment| i + 22 + comment == tail.len())
        })
        .ok_or_else(damaged)?;
    let count = read_u16(&tail, end + 10).ok_or_else(damaged)?;
    let directory_len = read_u32(&tail, end + 12).ok_or_else(damaged)? as usize;
    let offset = u64::from(read_u32(&tail, end + 16).ok_or_else(damaged)?);
    if read_u16(&tail, end + 4) != Some(0)
        || read_u16(&tail, end + 6) != Some(0)
        || read_u16(&tail, end + 8) != Some(count)
        || count > MAX_ENTRIES
        || directory_len > MAX_DIRECTORY_BYTES
        || offset + directory_len as u64 > len - tail_len as u64 + end as u64
    {
        return Err(damaged());
    }
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(|_| damaged())?;
    let mut directory = vec![0; directory_len];
    reader.read_exact(&mut directory).map_err(|_| damaged())?;
    let b = directory.as_slice();
    let mut at = 0;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if b.get(at..at + 4) != Some(b"PK\x01\x02".as_slice()) {
            return Err(DAMAGED.to_owned());
        }
        let method = read_u16(b, at + 10).ok_or_else(|| DAMAGED.to_owned())? as u16;
        let crc = read_u32(b, at + 16).ok_or_else(|| DAMAGED.to_owned())?;
        let packed = u64::from(read_u32(b, at + 20).ok_or_else(|| DAMAGED.to_owned())?);
        let size = u64::from(read_u32(b, at + 24).ok_or_else(|| DAMAGED.to_owned())?);
        let name_len = read_u16(b, at + 28).ok_or_else(|| DAMAGED.to_owned())?;
        let extra_len = read_u16(b, at + 30).ok_or_else(|| DAMAGED.to_owned())?;
        let comment_len = read_u16(b, at + 32).ok_or_else(|| DAMAGED.to_owned())?;
        let local_offset = u64::from(read_u32(b, at + 42).ok_or_else(|| DAMAGED.to_owned())?);
        if read_u16(b, at + 8).is_none_or(|flags| flags & 1 != 0) || local_offset + 30 > offset {
            return Err(damaged());
        }
        let name_bytes = b
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(|| DAMAGED.to_owned())?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        out.push(ZipEntry {
            name,
            method,
            crc,
            packed,
            size,
            local_offset,
        });
        at = at
            .saturating_add(46)
            .saturating_add(name_len)
            .saturating_add(extra_len)
            .saturating_add(comment_len);
    }
    Ok(out)
}

#[derive(Default)]
struct ZipWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl ZipWriter {
    fn add(
        &mut self,
        name: &str,
        method: u16,
        data: &[u8],
        crc: u32,
        size: u64,
    ) -> Result<(), String> {
        let too_big = || "Arquivo excede o limite de 4 GiB do contêiner ZIP.".to_owned();
        let offset = u32::try_from(self.out.len()).map_err(|_| too_big())?;
        let packed = u32::try_from(data.len()).map_err(|_| too_big())?;
        let size = u32::try_from(size).map_err(|_| too_big())?;
        let make_header = |central: bool| {
            let mut h = Vec::with_capacity(46 + name.len());
            h.extend(if central { 0x0201_4b50u32 } else { 0x0403_4b50 }.to_le_bytes());
            if central {
                h.extend(20u16.to_le_bytes());
            }
            h.extend(20u16.to_le_bytes());
            h.extend(0x0800u16.to_le_bytes()); // UTF-8 filename flag
            h.extend(method.to_le_bytes());
            h.extend([0u8; 4]);
            h.extend(crc.to_le_bytes());
            h.extend(packed.to_le_bytes());
            h.extend(size.to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes());
            if central {
                h.extend([0u8; 6]);
                h.extend(0u32.to_le_bytes());
                h.extend(offset.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        self.out.extend(make_header(false));
        self.out.extend_from_slice(data);
        self.central.extend(make_header(true));
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>, String> {
        let cd_offset = u32::try_from(self.out.len())
            .map_err(|_| "Arquivo excede o limite de 4 GiB do contêiner ZIP.".to_owned())?;
        let cd_len = self.central.len() as u32;
        self.out.append(&mut self.central);
        self.out.extend(0x0605_4b50u32.to_le_bytes());
        self.out.extend([0u8; 4]);
        self.out.extend(self.count.to_le_bytes());
        self.out.extend(self.count.to_le_bytes());
        self.out.extend(cd_len.to_le_bytes());
        self.out.extend(cd_offset.to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        Ok(self.out)
    }
}

/// Renders a crisp, fast 320×200 PNG floor-plan preview thumbnail directly in
/// software (~1 ms) for embedding inside `.trazo`/`.trazot` containers.
pub fn render_thumbnail_png(project: &Project) -> Option<Vec<u8>> {
    let (mut min, mut max) = project
        .content_bounds()
        .or_else(|| project.terrain.as_ref().map(|t| (t.min, t.max)))?;
    if let Some(t) = &project.terrain {
        min.x = min.x.min(t.min.x);
        min.y = min.y.min(t.min.y);
        max.x = max.x.max(t.max.x);
        max.y = max.y.max(t.max.y);
    }
    let span_x = (max.x - min.x).max(0.5);
    let span_y = (max.y - min.y).max(0.5);
    let width = 320u32;
    let height = 200u32;
    let pad = 16.0f64;
    let scale = ((width as f64 - 2.0 * pad) / span_x).min((height as f64 - 2.0 * pad) / span_y);
    let ox = (width as f64 - span_x * scale) * 0.5;
    let oy = (height as f64 - span_y * scale) * 0.5;

    let to_px = |x: f64, y: f64| -> (i32, i32) {
        (
            (ox + (x - min.x) * scale).round() as i32,
            (oy + (y - min.y) * scale).round() as i32,
        )
    };

    let bg = Rgba([248, 250, 252, 255]);
    let grid = Rgba([234, 239, 246, 255]);
    let terrain_col = Rgba([148, 163, 184, 255]);
    let room_fill = Rgba([232, 241, 252, 255]);
    let wall_col = Rgba([30, 41, 59, 255]);
    let accent_col = Rgba([37, 99, 235, 255]);

    let mut img = RgbaImage::from_pixel(width, height, bg);
    // Subtle background grid
    for gx in (0..width).step_by(20) {
        for gy in 0..height {
            img.put_pixel(gx, gy, grid);
        }
    }
    for gy in (0..height).step_by(20) {
        for gx in 0..width {
            img.put_pixel(gx, gy, grid);
        }
    }

    // Room bounding fills
    for room in &project.rooms {
        if let Some((rmin, rmax)) = project.room_bounds(room) {
            let (x0, y0) = to_px(rmin.x, rmin.y);
            let (x1, y1) = to_px(rmax.x, rmax.y);
            fill_rect(&mut img, x0, y0, x1, y1, room_fill);
        }
    }

    // Terrain outline
    if let Some(terrain) = &project.terrain {
        let outline = terrain.outline();
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            let p0 = to_px(a.x, a.y);
            let p1 = to_px(b.x, b.y);
            draw_thick_line(&mut img, p0, p1, 1, terrain_col);
        }
    }

    // Walls
    for wall in &project.walls {
        let p0 = to_px(wall.start.x, wall.start.y);
        let p1 = to_px(wall.end.x, wall.end.y);
        draw_thick_line(&mut img, p0, p1, 2, wall_col);
    }

    // Fixtures & columns as subtle accent marks
    for f in &project.fixtures {
        let (fmin, fmax) = f.bounds();
        let (x0, y0) = to_px(fmin.x, fmin.y);
        let (x1, y1) = to_px(fmax.x, fmax.y);
        fill_rect(&mut img, x0, y0, x1.max(x0 + 2), y1.max(y0 + 2), accent_col);
    }

    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, ImageFormat::Png)
        .ok()?;
    Some(buf.into_inner())
}

fn fill_rect(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let xa = x0.min(x1).clamp(0, w.saturating_sub(1));
    let xb = x0.max(x1).clamp(0, w.saturating_sub(1));
    let ya = y0.min(y1).clamp(0, h.saturating_sub(1));
    let yb = y0.max(y1).clamp(0, h.saturating_sub(1));
    for y in ya..=yb {
        for x in xa..=xb {
            img.put_pixel(x as u32, y as u32, color);
        }
    }
}

fn draw_thick_line(
    img: &mut RgbaImage,
    p0: (i32, i32),
    p1: (i32, i32),
    thickness: i32,
    color: Rgba<u8>,
) {
    let dx = (p1.0 - p0.0).abs();
    let dy = (p1.1 - p0.1).abs();
    let steps = dx.max(dy).max(1);
    let r = thickness / 2;
    for s in 0..=steps {
        let t = s as f64 / steps as f64;
        let x = (p0.0 as f64 + (p1.0 - p0.0) as f64 * t).round() as i32;
        let y = (p0.1 as f64 + (p1.1 - p0.1) as f64 * t).round() as i32;
        fill_rect(img, x - r, y - r, x + r, y + r, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::geometry::Point;

    #[test]
    fn parallel_zip_round_trips_compresses_over_3x_and_embeds_thumbnail() {
        let mut project = Project::default();
        project.name = "Torre Teste".to_owned();
        for i in 0..400usize {
            let x = (i % 20) as f64 * 3.0;
            let y = (i / 20) as f64 * 3.0;
            project.add_wall(Point::new(x, y), Point::new(x + 2.5, y));
        }
        let json = serde_json::to_string_pretty(&project).unwrap();
        let thumb = render_thumbnail_png(&project).unwrap();
        assert!(thumb.starts_with(b"\x89PNG"));

        let packed = pack_project(&json, Some(&thumb), 6).unwrap();
        assert!(is_zip(&packed));
        assert!(
            packed.len() * 3 < json.len(),
            "compressed ZIP ({} B) should be >3x smaller than JSON ({} B)",
            packed.len(),
            json.len()
        );
        assert_eq!(inflated_size(&packed), Some(json.len() as u64));

        // Thumbnail readable directly without inflating project.json.
        assert_eq!(read_thumbnail_from_bytes(&packed), Some(thumb));

        // Full unpack matches byte for byte.
        let unpacked = unpack_project(&packed, 64 << 20).unwrap();
        assert_eq!(unpacked, json);

        // Plain JSON transparently passes through.
        let plain_out = unpack_project(json.as_bytes(), 64 << 20).unwrap();
        assert_eq!(plain_out, json);

        // Zip-bomb limit refuses oversized declared size cleanly.
        assert!(unpack_project(&packed, 100).is_err());
    }
    #[test]
    fn stored_entries_reject_forged_sizes_before_copying_data() {
        for name in [PROJECT_ENTRY, THUMBNAIL_ENTRY] {
            let mut zip = ZipWriter::default();
            zip.add(name, 0, &[0; 1024], 0, 1).unwrap();
            let bytes = zip.finish().unwrap();
            let entry = parse_entries(&bytes).unwrap().remove(0);
            assert!(entry.data(&bytes).is_err());
            assert!(unpack_project(&bytes, 100).is_err());
            assert!(read_thumbnail_from_bytes(&bytes).is_none());
        }
    }

    #[test]
    fn thumbnail_reads_only_directory_and_preview() {
        struct CountedReader {
            cursor: Cursor<Vec<u8>>,
            read_bytes: usize,
        }
        impl Read for CountedReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let count = self.cursor.read(buf)?;
                self.read_bytes += count;
                Ok(count)
            }
        }
        impl Seek for CountedReader {
            fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
                self.cursor.seek(pos)
            }
        }

        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1, 1)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let thumb = png.into_inner();
        let mut crc = Crc::new();
        crc.update(&thumb);
        let mut zip = ZipWriter::default();
        zip.add(PROJECT_ENTRY, 0, &vec![0; 2 << 20], 0, 2 << 20)
            .unwrap();
        zip.add(THUMBNAIL_ENTRY, 0, &thumb, crc.sum(), thumb.len() as u64)
            .unwrap();
        let mut reader = CountedReader {
            cursor: Cursor::new(zip.finish().unwrap()),
            read_bytes: 0,
        };
        assert_eq!(
            read_thumbnail_from_reader(&mut reader).as_deref(),
            Some(thumb.as_slice())
        );
        assert!(
            reader.read_bytes < 70_000,
            "project payload must not be read"
        );
    }

    #[test]
    fn thumbnail_rejects_oversized_pixel_dimensions() {
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1025, 1)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let bytes = pack_project("{}", Some(&png.into_inner()), 6).unwrap();
        assert!(read_thumbnail_from_bytes(&bytes).is_none());
    }
}
