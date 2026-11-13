//! Keeps denormal (subnormal) floats out of the audio thread.
//!
//! A filter fed silence decays towards zero through the subnormal range, where x86 CPUs
//! take a slow path costing tens of times more per operation, and its state can settle
//! there for good. That makes the engine most expensive exactly when a deck goes quiet.
//! While a [`NoDenormals`] guard is alive the CPU rounds such values to zero instead,
//! which is inaudible (they are below −750 dBFS).

/// Flush-to-zero (bit 15) and denormals-are-zero (bit 6) in the x86 MXCSR register.
#[cfg(target_arch = "x86_64")]
const FTZ_DAZ: u32 = 0x8040;

/// While alive, the current thread treats denormal floats as zero. Dropping it restores
/// the thread's previous floating-point mode. Does nothing on other architectures.
pub struct NoDenormals {
    #[cfg(target_arch = "x86_64")]
    saved: u32,
}

impl NoDenormals {
    #[inline]
    pub fn enter() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let saved = read_mxcsr();
            write_mxcsr(saved | FTZ_DAZ);
            Self { saved }
        }
        #[cfg(not(target_arch = "x86_64"))]
        Self {}
    }
}

impl Drop for NoDenormals {
    #[inline]
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        write_mxcsr(self.saved);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn read_mxcsr() -> u32 {
    let mut csr = 0u32;
    // SAFETY: `stmxcsr` writes the 32-bit control register to the given address, which
    // points at a live `u32`. SSE is part of the x86_64 baseline.
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
    }
    csr
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn write_mxcsr(csr: u32) {
    // SAFETY: `ldmxcsr` loads the control register from a live `u32`. Only the two
    // denormal-handling bits ever differ from the value read on this thread, so rounding
    // and exception masks are untouched.
    unsafe {
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, readonly, preserves_flags));
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::hint::black_box;

    fn halve_smallest_normal() -> f32 {
        black_box(f32::MIN_POSITIVE) * black_box(0.5)
    }

    #[test]
    fn denormals_become_zero_only_while_the_guard_is_alive() {
        assert!(halve_smallest_normal() > 0.0);
        {
            let _guard = NoDenormals::enter();
            assert_eq!(halve_smallest_normal(), 0.0);
        }
        assert!(halve_smallest_normal() > 0.0);
    }
}
