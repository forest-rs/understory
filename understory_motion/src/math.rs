// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!("understory_motion requires either the `std` or `libm` feature for float math");

#[cfg(feature = "std")]
mod backend {
    #[inline]
    pub(crate) fn cos(value: f64) -> f64 {
        value.cos()
    }

    #[inline]
    pub(crate) fn exp(value: f64) -> f64 {
        value.exp()
    }

    #[inline]
    pub(crate) fn rem_euclid(value: f64, rhs: f64) -> f64 {
        value.rem_euclid(rhs)
    }

    #[inline]
    pub(crate) fn sin(value: f64) -> f64 {
        value.sin()
    }

    #[inline]
    pub(crate) fn sqrt(value: f64) -> f64 {
        value.sqrt()
    }
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
mod backend {
    #[inline]
    pub(crate) fn cos(value: f64) -> f64 {
        libm::cos(value)
    }

    #[inline]
    pub(crate) fn exp(value: f64) -> f64 {
        libm::exp(value)
    }

    #[inline]
    pub(crate) fn rem_euclid(value: f64, rhs: f64) -> f64 {
        let result = value % rhs;
        if result < 0.0 {
            result + rhs.abs()
        } else {
            result
        }
    }

    #[inline]
    pub(crate) fn sin(value: f64) -> f64 {
        libm::sin(value)
    }

    #[inline]
    pub(crate) fn sqrt(value: f64) -> f64 {
        libm::sqrt(value)
    }
}

pub(crate) use backend::{cos, exp, rem_euclid, sin, sqrt};
