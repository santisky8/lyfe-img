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

First use (2026-10-18): the 9 static homepage photos went from 858 KB of 800 px JPEGs to
390 KB of 960 px AVIF on desktop (54% smaller, sharper) and 144 KB on phones (83% smaller).
Encoding all 9 at two widths and two formats took about 4 s using every CPU core (rayon).
