mod canvas;

use {
    self::canvas::Canvas,
    rand::{distributions::uniform::Uniform, Rng},
    rustfft::{algorithm::Radix4, num_complex::Complex, Fft, FftDirection},
    std::{f32::consts::TAU, path::Path},
};

fn noise_1d_freq(freq: f32, rng: &mut impl Rng) -> impl Iterator<Item = f32> {
    let phase = rng.sample(Uniform::new(0.0, TAU));
    (0u64..).map(move |index| (TAU * freq * index as f32 + phase).sin())
}

pub fn nosie_1d<P>(
    parameters: impl Iterator<Item = P>,
    mut freq: impl FnMut(&P) -> f32,
    mut amp: impl FnMut(&P) -> f32,
    rng: &mut impl Rng,
) -> impl Iterator<Item = f32> {
    let mut sum_amp = 0.0;
    let mut iterators: Vec<_> = parameters
        .map(|p| {
            let f = freq(&p);
            let a = amp(&p);
            sum_amp += a;
            (noise_1d_freq(f, rng), a)
        })
        .collect();

    for (_, amp) in &mut iterators {
        *amp /= sum_amp;
    }

    core::iter::repeat_with(move || {
        iterators.iter_mut().fold(0.0, |acc, (iter, a)| {
            let s = iter.next().unwrap();
            acc + s * *a
        })
    })
}

pub fn noise_1d_with_amp_fn(
    freqs: impl Iterator<Item = f32>,
    mut amp: impl FnMut(f32) -> f32,
    rng: &mut impl Rng,
) -> impl Iterator<Item = f32> {
    nosie_1d(freqs, |&f| f, move |&f| amp(f), rng)
}

pub fn noise_1d_with_exp(
    freqs: impl Iterator<Item = f32>,
    exp: f32,
    rng: &mut impl Rng,
) -> impl Iterator<Item = f32> {
    nosie_1d(freqs, |&f| f, move |&f| f.powf(exp), rng)
}

pub fn generate_blue_noise(
    size: usize,
    fmin: f32,
    fmax: f32,
    exp: f32,
) -> Canvas<Complex<f32>> {
    let mut canvas = Canvas::new(size, Complex { re: 0.0, im: 0.0 });

    for i in 0..size {
        for j in 0..size {
            let x = i as f32 - (size / 2) as f32;
            let y = j as f32 - (size / 2) as f32;
            let d = (x * x + y * y).sqrt();
            if d < fmin || d > fmax {
                continue;
            }

            let a = ((d - fmin) / (fmax - fmin)).powf(exp);

            // let noise = rand::random::<f32>();
            // canvas.pixel(i, j).re = noise * a;
            // let noise = rand::random::<f32>();
            // canvas.pixel(i, j).im = noise * a;

            let noise = rand::random::<f32>();
            canvas
                .pixel((i + size / 2) % size, (j + size / 2) % size)
                .re = noise * a;
            let noise = rand::random::<f32>();
            canvas
                .pixel((i + size / 2) % size, (j + size / 2) % size)
                .re = noise * a;
        }
    }

    *canvas.pixel(0, 0) = Complex { re: 1.0, im: 1.0 };

    // save_tmp_image(&canvas, "freq.png");

    // let mut planner = FftPlanner::new();
    // let fft = planner.plan_fft_inverse(size);
    let fft = Radix4::new(size, FftDirection::Inverse);
    // let fft = Dft::new(size, FftDirection::Inverse);
    let mut scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];

    for row in canvas.rows_mut() {
        fft.process_with_scratch(row, &mut scratch);
    }

    // save_tmp_image(&canvas, "fft_row.png");

    canvas.transpose();

    for row in canvas.rows_mut() {
        fft.process_with_scratch(row, &mut scratch);
    }

    // save_tmp_image(&canvas, "fft_row_column.png");

    canvas
}

// fn save_tmp_image(canvas: &Canvas<Complex<f32>>, path: impl AsRef<Path>) {
//     let max = canvas.values().fold(0.0, |a, c| c.re.max(a));
//     let avg = canvas.values().fold(0.0, |a, c| c.re + a)
//         / canvas.size() as f32
//         / canvas.size() as f32;

//     eprintln!("Max = {}, Avg = {}", max, avg);

//     canvas
//         .map_ref(|c| normalize_8(c.re))
//         .save_to_image(path)
//         .unwrap();
// }

pub fn save_rgba8_image(
    canvas_red: &Canvas<Complex<f32>>,
    canvas_green: &Canvas<Complex<f32>>,
    canvas_blue: &Canvas<Complex<f32>>,
    canvas_alpha: &Canvas<Complex<f32>>,
    path: impl AsRef<Path>,
) -> Result<(), image::ImageError> {
    use image::{save_buffer_with_format, ColorType, ImageFormat};

    assert_eq!(canvas_red.size(), canvas_green.size());
    assert_eq!(canvas_red.size(), canvas_blue.size());
    assert_eq!(canvas_red.size(), canvas_alpha.size());

    let iter = canvas_red
        .values()
        .zip(canvas_green.values())
        .zip(canvas_blue.values())
        .zip(canvas_alpha.values());

    let mut bytes = Vec::new();
    for (((r, g), b), a) in iter {
        bytes.push(normalize_8(r.re));
        bytes.push(normalize_8(g.re));
        bytes.push(normalize_8(b.re));
        bytes.push(normalize_8(a.re));
    }

    save_buffer_with_format(
        path,
        &bytes,
        canvas_red.size() as u32,
        canvas_red.size() as u32,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
}

pub fn save_rgba16_image(
    canvas_red: &Canvas<Complex<f32>>,
    canvas_green: &Canvas<Complex<f32>>,
    canvas_blue: &Canvas<Complex<f32>>,
    canvas_alpha: &Canvas<Complex<f32>>,
    path: impl AsRef<Path>,
) -> Result<(), image::ImageError> {
    use image::{save_buffer_with_format, ColorType, ImageFormat};

    assert_eq!(canvas_red.size(), canvas_green.size());
    assert_eq!(canvas_red.size(), canvas_blue.size());
    assert_eq!(canvas_red.size(), canvas_alpha.size());

    let iter = canvas_red
        .values()
        .zip(canvas_green.values())
        .zip(canvas_blue.values())
        .zip(canvas_alpha.values());

    let mut bytes = Vec::new();
    for (((r, g), b), a) in iter {
        bytes.extend_from_slice(&normalize_16(r.re).to_be_bytes());
        bytes.extend_from_slice(&normalize_16(g.re).to_be_bytes());
        bytes.extend_from_slice(&normalize_16(b.re).to_be_bytes());
        bytes.extend_from_slice(&normalize_16(a.re).to_be_bytes());
    }

    save_buffer_with_format(
        path,
        &bytes,
        canvas_red.size() as u32,
        canvas_red.size() as u32,
        ColorType::Rgba16,
        ImageFormat::Png,
    )
}

fn normalize_8(value: f32) -> u8 {
    (value.abs().fract() * 255.0) as u8
}

fn normalize_16(value: f32) -> u16 {
    (value.abs().fract() * 65535.0) as u16
}
