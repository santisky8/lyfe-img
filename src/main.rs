//! lyfe-img: turn source photos into small, responsive AVIF and WebP files for lyfehealth.ca.
//!
//! Each input becomes one file per width and format (for example `hero-480.avif`,
//! `hero-480.webp`, ...), never upscaled. A `manifest.json` lists what was written so the
//! website can build `<picture>` / `srcset` markup, and a size report compares the result
//! with the original download.
//!
//! Inputs are `name=source`, where source is a local path or an http(s) URL:
//!   lyfe-img --out public/images/home hero=https://images.pexels.com/photos/1/x.jpeg

use anyhow::{bail, Context, Result};
use clap::Parser;
use image::codecs::avif::AvifEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about = "Responsive AVIF + WebP images for lyfehealth.ca")]
struct Args {
    /// Inputs as name=path-or-url (name becomes the output file prefix)
    #[arg(required = true)]
    inputs: Vec<String>,
    /// Output directory
    #[arg(long, default_value = "out")]
    out: PathBuf,
    /// Comma-separated output widths in pixels (larger than the source are skipped)
    #[arg(long, default_value = "480,960,1600", value_delimiter = ',')]
    widths: Vec<u32>,
    /// AVIF quality 1-100 (AVIF looks good at lower numbers than WebP)
    #[arg(long, default_value_t = 55)]
    avif_quality: u8,
    /// WebP quality 1-100
    #[arg(long, default_value_t = 75.0)]
    webp_quality: f32,
    /// AVIF encoder speed 1 (smallest, slowest) to 10 (fastest)
    #[arg(long, default_value_t = 6)]
    speed: u8,
}

/// One decoded source image plus how many bytes the original download/file was.
struct Source {
    name: String,
    img: DynamicImage,
    original_bytes: usize,
}

fn load(spec: &str) -> Result<Source> {
    let (name, src) = spec
        .split_once('=')
        .with_context(|| format!("input must be name=path-or-url, got {spec}"))?;
    if name.is_empty() || name.contains('/') {
        bail!("bad name {name:?}: use a short file-safe name");
    }
    let bytes = if src.starts_with("http://") || src.starts_with("https://") {
        ureq::get(src)
            .call()
            .with_context(|| format!("download {src}"))?
            .body_mut()
            .with_config()
            .limit(60 * 1024 * 1024)
            .read_to_vec()?
    } else {
        fs::read(src).with_context(|| format!("read {src}"))?
    };
    let img = image::load_from_memory(&bytes).with_context(|| format!("decode {src}"))?;
    Ok(Source { name: name.to_string(), img, original_bytes: bytes.len() })
}

/// Encode one resized copy as AVIF and WebP; returns (format, path, bytes) for each.
fn encode(src: &Source, width: u32, args: &Args) -> Result<Vec<(&'static str, PathBuf, usize)>> {
    let (w0, h0) = (src.img.width(), src.img.height());
    let height = ((h0 as f64) * (width as f64) / (w0 as f64)).round() as u32;
    let resized = src.img.resize_exact(width, height, FilterType::Lanczos3);
    let rgb = DynamicImage::ImageRgb8(resized.to_rgb8());

    let avif_path = args.out.join(format!("{}-{width}.avif", src.name));
    let mut avif = Vec::new();
    rgb.write_with_encoder(AvifEncoder::new_with_speed_quality(&mut avif, args.speed, args.avif_quality))?;
    fs::write(&avif_path, &avif)?;

    let webp_path = args.out.join(format!("{}-{width}.webp", src.name));
    let webp = webp::Encoder::from_image(&rgb)
        .map_err(|e| anyhow::anyhow!("webp encoder: {e}"))?
        .encode(args.webp_quality);
    fs::write(&webp_path, &*webp)?;

    Ok(vec![("avif", avif_path, avif.len()), ("webp", webp_path, webp.len())])
}

fn kb(n: usize) -> String {
    format!("{:.0} KB", n as f64 / 1024.0)
}

fn rel(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

fn main() -> Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    // Downloads and decodes run in parallel, then every (image, width) pair encodes in parallel.
    let sources: Vec<Source> = args.inputs.par_iter().map(|s| load(s)).collect::<Result<_>>()?;
    let jobs: Vec<(&Source, u32)> = sources
        .iter()
        .flat_map(|s| {
            let mut ws: Vec<u32> = args.widths.iter().copied().filter(|&w| w <= s.img.width()).collect();
            if ws.is_empty() {
                ws.push(s.img.width()); // small source: keep its own width, never upscale
            }
            ws.into_iter().map(move |w| (s, w))
        })
        .collect();
    let results: Vec<(String, u32, Vec<(&str, PathBuf, usize)>)> = jobs
        .par_iter()
        .map(|(s, w)| Ok((s.name.clone(), *w, encode(s, *w, &args)?)))
        .collect::<Result<_>>()?;

    // Manifest: name -> source size and the files per format, smallest width first.
    let mut manifest: BTreeMap<String, serde_like::Entry> = BTreeMap::new();
    for s in &sources {
        manifest.insert(s.name.clone(), serde_like::Entry::new(s.img.width(), s.img.height()));
    }
    for (name, w, files) in &results {
        let e = manifest.get_mut(name).unwrap();
        for (fmt, path, bytes) in files {
            e.add(fmt, *w, &rel(path), *bytes);
        }
    }
    fs::write(args.out.join("manifest.json"), serde_like::to_json(&manifest))?;

    // Report: original vs the file a phone (smallest width) and a desktop (largest) would load.
    println!("{:<14} {:>10} {:>16} {:>16}", "image", "original", "phone (avif)", "desktop (avif)");
    let (mut orig_total, mut desk_total) = (0usize, 0usize);
    for s in &sources {
        let e = &manifest[&s.name];
        let (small, large) = (e.avif.first().unwrap(), e.avif.last().unwrap());
        orig_total += s.original_bytes;
        desk_total += large.2;
        println!(
            "{:<14} {:>10} {:>10} @{:<4} {:>10} @{:<4}",
            s.name, kb(s.original_bytes), kb(small.2), small.0, kb(large.2), large.0
        );
    }
    let saved = 100.0 * (1.0 - desk_total as f64 / orig_total.max(1) as f64);
    println!("total original {} -> desktop avif {} ({saved:.0}% smaller)", kb(orig_total), kb(desk_total));
    Ok(())
}

/// Tiny hand-written JSON output so the tool needs no serde dependency.
mod serde_like {
    use std::collections::BTreeMap;

    pub struct Entry {
        pub width: u32,
        pub height: u32,
        pub avif: Vec<(u32, String, usize)>,
        pub webp: Vec<(u32, String, usize)>,
    }

    impl Entry {
        pub fn new(width: u32, height: u32) -> Self {
            Entry { width, height, avif: vec![], webp: vec![] }
        }
        pub fn add(&mut self, fmt: &str, w: u32, file: &str, bytes: usize) {
            let list = if fmt == "avif" { &mut self.avif } else { &mut self.webp };
            list.push((w, file.to_string(), bytes));
            list.sort_by_key(|x| x.0);
        }
    }

    fn files(list: &[(u32, String, usize)]) -> String {
        let items: Vec<String> = list
            .iter()
            .map(|(w, f, b)| format!("{{\"w\":{w},\"file\":\"{f}\",\"bytes\":{b}}}"))
            .collect();
        format!("[{}]", items.join(","))
    }

    pub fn to_json(m: &BTreeMap<String, Entry>) -> String {
        let items: Vec<String> = m
            .iter()
            .map(|(k, e)| {
                format!(
                    "  \"{k}\": {{\"width\":{},\"height\":{},\"avif\":{},\"webp\":{}}}",
                    e.width, e.height, files(&e.avif), files(&e.webp)
                )
            })
            .collect();
        format!("{{\n{}\n}}\n", items.join(",\n"))
    }
}
