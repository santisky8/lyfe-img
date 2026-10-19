# lyfe-img

Rust command-line tool that turns source photos into small, responsive AVIF and WebP files
for lyfehealth.ca. Each input becomes one file per width and format (never upscaled), plus a
`manifest.json` and a size report.

```sh
cargo build --release
./target/release/lyfe-img --out public/images/home --widths 480,960 \
  'hero=https://images.pexels.com/photos/5966434/pexels-photo-5966434.jpeg?auto=compress&w=1920'
```

Options: `--widths` (default 480,960,1600), `--avif-quality` (55), `--webp-quality` (75),
`--speed` (AVIF encoder, 1 slowest/smallest to 10 fastest; default 6).

## Results

First use (2026-10-18), on the 9 static photos of the lyfehealth.ca homepage. Before, the
site served 800 px JPEGs. Totals for all 9 photos:

| Version | Total size | Change vs JPEG |
|---|---:|---:|
| Before: 800 px JPEG | 858 KB | baseline |
| Desktop: 960 px AVIF | 391 KB | 54% smaller, and sharper |
| Desktop fallback: 960 px WebP | 674 KB | 21% smaller |
| Phone: 480 px AVIF | 145 KB | 83% smaller |
| Phone fallback: 480 px WebP | 246 KB | 71% smaller |

Encoding all 9 photos at two widths and two formats took about 4 s, using every CPU core
(rayon).

## License

MIT. See [LICENSE](LICENSE).
