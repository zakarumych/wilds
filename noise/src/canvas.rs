pub struct Canvas<T> {
    size: usize,
    values: Vec<T>,
}

impl<T> Canvas<T> {
    pub fn new(size: usize, value: T) -> Self
    where
        T: Copy,
    {
        Canvas {
            size,
            values: vec![value; size * size],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn row(&self, n: usize) -> &[T] {
        let start = self.size * n;
        &self.values[start..start + self.size]
    }

    pub fn rows(&self) -> impl Iterator<Item = &[T]> {
        self.values.chunks_exact(self.size)
    }

    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut [T]> {
        self.values.chunks_exact_mut(self.size)
    }

    pub fn row_mut(&mut self, n: usize) -> &mut [T] {
        let start = self.size * n;
        &mut self.values[start..start + self.size]
    }

    pub fn pixel(&mut self, x: usize, y: usize) -> &mut T {
        &mut self.values[x + y * self.size]
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.values.iter()
    }

    pub fn transpose(&mut self) {
        for i in 0..self.size {
            for j in 0..=i {
                self.values.swap(i + j * self.size, j + i * self.size);
            }
        }
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Canvas<U> {
        Canvas {
            size: self.size,
            values: self.values.into_iter().map(f).collect(),
        }
    }

    pub fn map_ref<U>(&self, f: impl FnMut(&T) -> U) -> Canvas<U> {
        Canvas {
            size: self.size,
            values: self.values.iter().map(f).collect(),
        }
    }
}

impl Canvas<u8> {
    pub fn save_to_image(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), image::ImageError> {
        use image::{save_buffer_with_format, ColorType, ImageFormat};
        save_buffer_with_format(
            path,
            &self.values,
            self.size as u32,
            self.size as u32,
            ColorType::L8,
            ImageFormat::Png,
        )
    }
}
