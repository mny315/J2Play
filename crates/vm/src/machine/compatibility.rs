use super::{EmuError, HostServices, Method, vm_error};

pub(super) fn check_intrinsic_cancellation(host: &dyn HostServices) -> Result<(), EmuError> {
    if host.execution_cancelled() {
        Err(vm_error(
            "execution-cancelled",
            "intrinsic operation cancelled",
        ))
    } else {
        Ok(())
    }
}

pub(super) const LCDUI_GRAPHICS_SET_COLOR_FNV1A64: u64 = 0x42f2_428b_23fc_8e3f;
pub(super) const LCDUI_GRAPHICS_SET_RGB_COLOR_FNV1A64: u64 = 0x55ca_8797_97f5_8ac7;
pub(super) const LCDUI_GRAPHICS_TRANSLATE_FNV1A64: u64 = 0x4c7e_f6e3_edc5_22cd;
pub(super) const LCDUI_GRAPHICS_FILL_RECT_FNV1A64: u64 = 0x37bb_1b53_8ffd_bd52;
pub(super) const LCDUI_GRAPHICS_DRAW_RGB_FNV1A64: u64 = 0xfc18_f72a_4fec_4be2;
pub(super) const LCDUI_GRAPHICS_DRAW_IMAGE_FNV1A64: u64 = 0xbd57_5735_e139_808d;

pub(super) fn simple_case_unit(unit: u16, uppercase: bool) -> u16 {
    let Some(value) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mapped = natives::simple_case_mapping(value, uppercase);
    u16::try_from(u32::from(mapped)).unwrap_or(unit)
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub(super) fn compatibility_intrinsic_candidate(method: &Method) -> bool {
    method
        .compatibility_candidate
        .unwrap_or_else(|| classify_compatibility_candidate(method))
}

// All inputs are fixed after class linking. Synthetic methods can leave the
// cache unset while being assembled by conformance fixtures.
#[inline(never)]
fn classify_compatibility_candidate(method: &Method) -> bool {
    // Application code always uses the general bytecode engines. Only the
    // platform API implemented by our bootstrap may have native shortcuts.
    method.key.class.starts_with("java/") || method.key.class.starts_with("javax/")
}
