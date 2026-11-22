// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

/// Bit-level SWF reader (MSB-first within bytes, little-endian multi-byte integers).
pub struct BitReader<'a> {
    data: &'a [u8],
    pub byte_pos: usize,
    bit_buf: u8,
    bits_left: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], byte_pos: usize) -> Self {
        Self {
            data,
            byte_pos,
            bit_buf: 0,
            bits_left: 0,
        }
    }

    pub fn align(&mut self) {
        self.bits_left = 0;
    }

    pub fn read_ub(&mut self, mut n: u8) -> u32 {
        let mut val = 0u32;
        while n > 0 {
            if self.bits_left == 0 {
                if self.byte_pos >= self.data.len() {
                    return val << n;
                }
                self.bit_buf = self.data[self.byte_pos];
                self.byte_pos += 1;
                self.bits_left = 8;
            }
            let take = n.min(self.bits_left);
            let shift = self.bits_left - take;
            let mask = ((1u16 << take) - 1) as u8;
            let chunk = (self.bit_buf >> shift) & mask;
            val = (val << take) | (chunk as u32);
            self.bits_left -= take;
            n -= take;
        }
        val
    }

    pub fn read_sb(&mut self, n: u8) -> i32 {
        if n == 0 {
            return 0;
        }
        let u = self.read_ub(n);
        let sign = 1u32 << (n - 1);
        if (u & sign) != 0 {
            let extend = !((1u32 << n).wrapping_sub(1));
            (u | extend) as i32
        } else {
            u as i32
        }
    }

    pub fn skip_rect(&mut self) {
        self.align();
        let nb = self.read_ub(5) as u8;
        for _ in 0..4 {
            let _ = self.read_sb(nb);
        }
        self.align();
    }

    pub fn read_matrix(&mut self) -> Matrix2D {
        self.align();
        let (mut a, mut d) = (1.0f32, 1.0f32);
        let (mut b, mut c) = (0.0f32, 0.0f32);
        if self.read_ub(1) != 0 {
            let nb = self.read_ub(5) as u8;
            a = self.read_sb(nb) as f32 / 65536.0;
            d = self.read_sb(nb) as f32 / 65536.0;
        }
        if self.read_ub(1) != 0 {
            let nb = self.read_ub(5) as u8;
            b = self.read_sb(nb) as f32 / 65536.0;
            c = self.read_sb(nb) as f32 / 65536.0;
        }
        let nb = self.read_ub(5) as u8;
        let tx = self.read_sb(nb) as f32 / 20.0;
        let ty = self.read_sb(nb) as f32 / 20.0;
        self.align();
        Matrix2D { a, b, c, d, tx, ty }
    }

    pub fn skip_cxform_alpha(&mut self) {
        self.align();
        let has_add = self.read_ub(1) != 0;
        let has_mul = self.read_ub(1) != 0;
        let nb = self.read_ub(4) as u8;
        if has_mul {
            for _ in 0..4 {
                let _ = self.read_sb(nb);
            }
        }
        if has_add {
            for _ in 0..4 {
                let _ = self.read_sb(nb);
            }
        }
        self.align();
    }
}

/// 2D affine matrix in stage pixels (`x' = a*x + c*y + tx`, `y' = b*x + d*y + ty`).
#[derive(Debug, Clone, Copy)]
pub struct Matrix2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Matrix2D {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn mul(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            tx: self.a * child.tx + self.c * child.ty + self.tx,
            ty: self.b * child.tx + self.d * child.ty + self.ty,
        }
    }
}
