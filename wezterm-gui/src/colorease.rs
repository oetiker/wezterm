use crate::uniforms::{UniformBuilder, UniformStruct};
use config::EasingFunction;
use std::time::{Duration, Instant};

/// The wall-clock length of one animation frame at `animation_fps`.
///
/// L3/F4: this used to be spelled `1000 / fps` in integer milliseconds at two
/// independent sites — the timer that grants animation frames
/// (`TermWindow::schedule_animation_timer_if_needed`) and the ease that asks
/// for them (`ColorEase::intensity_one_shot`) — which at the shipped default
/// `animation_fps = 60` quantised both to 16 ms against a true 16.667 ms, and
/// collapsed every fps above 500 to 1 ms.  It lives here once so the two
/// clocks cannot disagree again; `fps` is expected pre-clamped to >= 1.
pub fn animation_frame_interval(fps: u64) -> Duration {
    Duration::from_secs_f64(1.0 / fps.max(1) as f64)
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ColorEase {
    in_duration: f32,
    in_function: EasingFunction,
    out_duration: f32,
    out_function: EasingFunction,
    start: Option<Instant>,
    last_render: Instant,
}

impl ColorEase {
    pub fn new(
        in_duration_ms: u64,
        in_function: EasingFunction,
        out_duration_ms: u64,
        out_function: EasingFunction,
        start: Option<Instant>,
    ) -> Self {
        Self {
            in_duration: Duration::from_millis(in_duration_ms).as_secs_f32(),
            in_function,
            out_duration: Duration::from_millis(out_duration_ms).as_secs_f32(),
            out_function,
            start,
            last_render: Instant::now(),
        }
    }

    pub fn update_start(&mut self, start: Instant) {
        let start = match self.start.take() {
            Some(prior) if prior >= start => prior,
            _ => start,
        };
        self.start.replace(start);
    }

    pub fn intensity(&mut self, is_one_shot: bool) -> Option<(f32, Instant)> {
        if is_one_shot {
            self.intensity_one_shot()
        } else {
            Some(self.intensity_continuous())
        }
    }

    /// Compute current intensity without side effects.
    /// Returns None if the cycle has completed (caller should
    /// trigger a real paint to restart the cycle).
    pub fn peek_intensity(&self) -> Option<f32> {
        let start = self.start?;
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed < self.in_duration {
            Some(
                self.in_function
                    .evaluate_at_position(elapsed / self.in_duration),
            )
        } else {
            let completion = (elapsed - self.in_duration) / self.out_duration;
            if completion >= 1.0 {
                None
            } else {
                Some(1.0 - self.out_function.evaluate_at_position(completion))
            }
        }
    }

    pub fn intensity_continuous(&mut self) -> (f32, Instant) {
        match self.intensity_one_shot() {
            Some(intensity) => intensity,
            None => {
                // Start a new cycle
                self.start.replace(Instant::now());
                self.intensity_one_shot().expect("just started")
            }
        }
    }

    pub fn intensity_one_shot(&mut self) -> Option<(f32, Instant)> {
        let start = self.start?;
        let elapsed = start.elapsed().as_secs_f32();

        let intensity = if elapsed < self.in_duration {
            Some(
                self.in_function
                    .evaluate_at_position(elapsed / self.in_duration),
            )
        } else {
            let completion = (elapsed - self.in_duration) / self.out_duration;
            if completion >= 1.0 {
                None
            } else {
                Some(1.0 - self.out_function.evaluate_at_position(completion))
            }
        };

        match intensity {
            Some(i) => {
                let now = Instant::now();
                let fps = if self.in_function == EasingFunction::Constant
                    && self.out_function == EasingFunction::Constant
                {
                    1
                } else {
                    config::configuration().animation_fps as u64
                };
                let next = match fps {
                    1 if elapsed < self.in_duration => {
                        start + Duration::from_secs_f32(self.in_duration)
                    }
                    1 => start + Duration::from_secs_f32(self.in_duration + self.out_duration),
                    _ => {
                        // F4 (the other half of L3): `1000 / fps` is integer
                        // division, so at the shipped default
                        // animation_fps = 60 this asked for a 16 ms frame
                        // while the timer that grants it runs at 16.667 ms
                        // (schedule_animation_timer_if_needed, corrected to
                        // Duration::from_secs_f64(1.0 / fps) in 230117c).
                        // This function produces the instants the renderer
                        // reuses as its clock, so the two must agree.
                        //
                        // The surrounding arithmetic went sub-millisecond
                        // with it rather than keeping an integer-millisecond
                        // modulo: quantising `elapsed` up to whole
                        // milliseconds only existed to make `%` work on
                        // u64, and rounding a phase up before taking it
                        // modulo a now-fractional interval would reintroduce
                        // the same class of error the fix removes.
                        let frame_interval = animation_frame_interval(fps);
                        let elapsed = Duration::from_secs_f32(elapsed);
                        // `remain` is the phase within the current frame.
                        // With exact arithmetic it is zero only if `elapsed`
                        // lands exactly on a frame boundary, which is now
                        // vanishingly unlikely rather than a 1-in-16 chance;
                        // the branch is kept because a zero phase still must
                        // not schedule a zero-length wait.  A sub-millisecond
                        // `remain` can now schedule an earlier wakeup than
                        // the old whole-millisecond one did, but at most one:
                        // `last_render` is set to `now` on every call, so the
                        // very next call inside this frame fails the
                        // `last_render.elapsed() >= frame_interval` guard and
                        // falls to the full-interval branch.
                        let remain = Duration::from_nanos(
                            (elapsed.as_nanos() % frame_interval.as_nanos()) as u64,
                        );
                        if !remain.is_zero() && self.last_render.elapsed() >= frame_interval {
                            now + remain
                        } else {
                            now + frame_interval
                        }
                    }
                };
                self.last_render = now;
                Some((i, next))
            }
            None => {
                self.start.take();
                None
            }
        }
    }
}

pub struct ColorEaseUniform {
    pub in_function: [f32; 4],
    pub out_function: [f32; 4],
    pub in_duration_ms: u32,
    pub out_duration_ms: u32,
}

impl From<ColorEase> for ColorEaseUniform {
    fn from(ease: ColorEase) -> ColorEaseUniform {
        Self {
            in_duration_ms: (ease.in_duration * 1000.).ceil() as u32,
            out_duration_ms: (ease.out_duration * 1000.).ceil() as u32,
            in_function: ease.in_function.as_bezier_array(),
            out_function: ease.out_function.as_bezier_array(),
        }
    }
}

impl<'a> UniformStruct<'a> for ColorEaseUniform {
    fn add_fields(&'a self, struct_name: &str, builder: &mut UniformBuilder<'a>) {
        builder.add_struct_field(struct_name, "in_function", &self.in_function);
        builder.add_struct_field(struct_name, "out_function", &self.out_function);
        builder.add_struct_field(struct_name, "in_duration_ms", &self.in_duration_ms);
        builder.add_struct_field(struct_name, "out_duration_ms", &self.out_duration_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_fps_is_not_quantised_to_whole_milliseconds() {
        // F4: the defect was `1000 / 60 == 16`. Pin both the value and the
        // fact that it is NOT the old one, so a revert to integer division
        // cannot pass.
        let interval = animation_frame_interval(60);
        assert_ne!(interval, Duration::from_millis(16));
        // 1/60 s rounded to the nearest nanosecond by from_secs_f64.
        assert_eq!(interval.as_nanos(), 16_666_667);
    }

    #[test]
    fn high_fps_no_longer_collapses_to_one_millisecond() {
        // `1000 / fps` gave 1 ms for every fps from 501 upwards, so 1000 and
        // 2000 fps were indistinguishable. They are not now.
        assert_eq!(animation_frame_interval(1000), Duration::from_micros(1000));
        assert_eq!(animation_frame_interval(2000), Duration::from_micros(500));
    }

    #[test]
    fn fps_zero_does_not_divide_by_zero() {
        // Both call sites clamp with `.max(1)` before calling, but the helper
        // is the thing that would produce an infinite Duration and panic in
        // from_secs_f64, so it clamps too.
        assert_eq!(animation_frame_interval(0), Duration::from_secs(1));
    }
}
