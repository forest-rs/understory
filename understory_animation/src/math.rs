// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!("understory_animation requires either the `std` or `libm` feature for float math");

#[cfg(feature = "std")]
mod backend {
    #[inline]
    pub(crate) fn ceil(value: f64) -> f64 {
        value.ceil()
    }

    #[inline]
    pub(crate) fn floor(value: f64) -> f64 {
        value.floor()
    }
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
mod backend {
    #[inline]
    pub(crate) fn ceil(value: f64) -> f64 {
        libm::ceil(value)
    }

    #[inline]
    pub(crate) fn floor(value: f64) -> f64 {
        libm::floor(value)
    }
}

pub(crate) use backend::{ceil, floor};
