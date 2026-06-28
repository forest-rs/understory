// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!(
    "understory_box_decoration requires either the `std` or `libm` feature for float math"
);

#[cfg(feature = "std")]
mod backend {
    #[inline]
    pub(crate) fn powf(value: f64, exponent: f64) -> f64 {
        value.powf(exponent)
    }
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
mod backend {
    #[inline]
    pub(crate) fn powf(value: f64, exponent: f64) -> f64 {
        libm::pow(value, exponent)
    }
}

pub(crate) use backend::powf;
