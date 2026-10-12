//! Sprite collision sampling directly from image storage, without frame copies.

use super::pixel_access::IntPixelSource;
use super::{
    EmuError, Handle, HeapValue, MAX_LCD_PIXELS, Machine, Value, check_raster_cancellation,
    heap_error, int_argument, reference_argument, type_error, vm_error,
};

#[derive(Clone, Copy)]
struct Frame {
    position: [i32; 2],
    size: [i32; 2],
    transform: i32,
}

#[derive(Clone, Copy)]
struct Bounds {
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
}

impl Bounds {
    fn intersect(self, other: Self) -> Option<Self> {
        let result = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (result.left < result.right && result.top < result.bottom).then_some(result)
    }
}

impl Frame {
    fn transform_bits(self) -> i32 {
        if (0..8).contains(&self.transform) {
            self.transform
        } else {
            0
        }
    }

    fn bounds(self, rectangle: [i32; 4]) -> Bounds {
        let [x, y, width, height] = rectangle;
        let mut x = [i64::from(x), i64::from(x) + i64::from(width) - 1];
        let mut y = [i64::from(y), i64::from(y) + i64::from(height) - 1];
        let transform = self.transform_bits();
        for (mask, size, endpoints) in [(2, self.size[0], &mut x), (1, self.size[1], &mut y)] {
            if transform & mask != 0 {
                *endpoints = endpoints.map(|value| i64::from(size.wrapping_sub(1)) - value);
            }
        }
        if transform & 4 != 0 {
            std::mem::swap(&mut x, &mut y);
        }
        Bounds {
            left: i64::from(self.position[0]) + x[0].min(x[1]),
            top: i64::from(self.position[1]) + y[0].min(y[1]),
            right: i64::from(self.position[0]) + x[0].max(x[1]) + 1,
            bottom: i64::from(self.position[1]) + y[0].max(y[1]) + 1,
        }
    }

    fn visual_bounds(self) -> Bounds {
        self.bounds([0, 0, self.size[0], self.size[1]])
    }
}

struct PixelScan<'a> {
    pixels: IntPixelSource<'a>,
    start: i64,
    step_x: i64,
    step_y: i64,
}

impl Machine<'_, '_> {
    pub(super) fn graphics_sprite_collision(&self, args: &[Value]) -> Result<bool, EmuError> {
        let sprite = reference_argument(args, 0)?;
        let col_w =
            self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.colW:I")?;
        let col_h =
            self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.colH:I")?;
        let other_w = int_argument(args, 11)?;
        let other_h = int_argument(args, 12)?;
        if col_w == 0 || col_h == 0 || other_w == 0 || other_h == 0 {
            return Ok(false);
        }

        let first = Frame {
            position: [
                self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Layer.x:I")?,
                self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Layer.y:I")?,
            ],
            size: [
                self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.fw:I")?,
                self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.fh:I")?,
            ],
            transform: self
                .graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.transform:I")?,
        };
        let second = Frame {
            position: [int_argument(args, 7)?, int_argument(args, 8)?],
            size: [int_argument(args, 4)?, int_argument(args, 5)?],
            transform: int_argument(args, 6)?,
        };
        let first_bounds = first.bounds([
            self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.colX:I")?,
            self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.colY:I")?,
            col_w,
            col_h,
        ]);
        let second_bounds = second.bounds([
            int_argument(args, 9)?,
            int_argument(args, 10)?,
            other_w,
            other_h,
        ]);
        let Some(bounds) = first_bounds.intersect(second_bounds) else {
            return Ok(false);
        };
        if int_argument(args, 13)? == 0 {
            return Ok(true);
        }

        let image = self.graphics_reference_field(
            sprite,
            "javax/microedition/lcdui/game/Sprite.image:Ljavax/microedition/lcdui/Image;",
        )?;
        let image_width =
            self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        if first.size[0] <= 0 || image_width < first.size[0] {
            return Err(vm_error("illegal-argument", "invalid Sprite frame width"));
        }
        let frame =
            self.graphics_int_field(sprite, "javax/microedition/lcdui/game/Sprite.frame:I")?;
        let frame = match self
            .heap
            .managed
            .field(sprite, "javax/microedition/lcdui/game/Sprite.sequence:[I")
            .map_err(heap_error)?
        {
            HeapValue::Reference(None) => frame,
            HeapValue::Reference(Some(sequence)) => match self
                .heap
                .managed
                .array_get(sequence, frame)
                .map_err(heap_error)?
            {
                HeapValue::Int(frame) => frame,
                _ => return Err(type_error()),
            },
            _ => return Err(type_error()),
        };
        let columns = image_width / first.size[0];
        let source = [
            (frame % columns).wrapping_mul(first.size[0]),
            (frame / columns).wrapping_mul(first.size[1]),
        ];

        // The collision rectangle may extend arbitrarily far outside the
        // images. Those pixels are transparent; exclude them before scanning.
        let Some(bounds) = bounds
            .intersect(first.visual_bounds())
            .and_then(|bounds| bounds.intersect(second.visual_bounds()))
        else {
            return Ok(false);
        };
        let first = self.sprite_pixel_scan(image, first, source, bounds)?;
        let second = self.sprite_pixel_scan(
            reference_argument(args, 1)?,
            second,
            [int_argument(args, 2)?, int_argument(args, 3)?],
            bounds,
        )?;
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let mut work = 0;
        for row in 0..height {
            let mut a = first.start + row * first.step_y;
            let mut b = second.start + row * second.step_y;
            for _ in 0..width {
                check_raster_cancellation(self.native_context, work)?;
                work += 1;
                if first.pixels.get(a as usize)?.cast_unsigned() >> 24 != 0
                    && second.pixels.get(b as usize)?.cast_unsigned() >> 24 != 0
                {
                    return Ok(true);
                }
                a += first.step_x;
                b += second.step_x;
            }
        }
        Ok(false)
    }

    fn sprite_pixel_scan(
        &self,
        image: Handle,
        frame: Frame,
        source: [i32; 2],
        bounds: Bounds,
    ) -> Result<PixelScan<'_>, EmuError> {
        let width = self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?;
        let height = self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?;
        if frame.size[0] <= 0
            || frame.size[1] <= 0
            || source[1] < 0
            || i64::from(source[1]) + i64::from(frame.size[1]) > i64::from(height)
        {
            return Err(vm_error("illegal-argument", "invalid Sprite source frame"));
        }
        let array =
            self.graphics_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")?;
        let pixels = self.graphics_int_array_source(array)?;
        let count =
            pixels.validate_region(width, source[0], source[1], frame.size[0], frame.size[1])?;
        if count > MAX_LCD_PIXELS {
            return Err(vm_error(
                "out-of-memory-error",
                "Sprite frame exceeds pixel limit",
            ));
        }

        let transform = frame.transform_bits();
        let mut x = bounds.left - i64::from(frame.position[0]);
        let mut y = bounds.top - i64::from(frame.position[1]);
        if transform & 4 != 0 {
            std::mem::swap(&mut x, &mut y);
        }
        let sign_x = if transform & 2 == 0 {
            1
        } else {
            x = i64::from(frame.size[0] - 1) - x;
            -1
        };
        let sign_y = if transform & 1 == 0 {
            1
        } else {
            y = i64::from(frame.size[1] - 1) - y;
            -1
        };
        let stride = i64::from(width);
        let (step_x, step_y) = if transform & 4 == 0 {
            (sign_x, sign_y * stride)
        } else {
            (sign_y * stride, sign_x)
        };
        Ok(PixelScan {
            pixels,
            start: (i64::from(source[1]) + y) * stride + i64::from(source[0]) + x,
            step_x,
            step_y,
        })
    }
}
