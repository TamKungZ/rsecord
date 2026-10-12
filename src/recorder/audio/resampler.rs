pub struct Resampler {
    pub ratio: f64,
    pub pos: f64,
    pub buf: Vec<[f32; 2]>,
}

impl Resampler {
    pub fn pull(&mut self, out: &mut Vec<i16>) {
        while (self.pos as usize) + 1 < self.buf.len() {
            let i = self.pos as usize;
            let f = (self.pos - i as f64) as f32;
            for c in 0..2 {
                let v = self.buf[i][c] * (1.0 - f) + self.buf[i + 1][c] * f;
                out.push((v.clamp(-1.0, 1.0) * 32767.0) as i16);
            }
            self.pos += self.ratio;
        }
        let drop = (self.pos as usize).min(self.buf.len());
        self.buf.drain(..drop);
        self.pos -= drop as f64;
    }
}