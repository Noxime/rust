mod float;
mod int;
mod uint;

pub use float::*;
pub use int::*;
pub use uint::*;

// Vectors of pointers are not for public use at the current time.
pub(crate) mod ptr;

use crate::{LaneCount, Mask, MaskElement, SupportedLaneCount};

/// A SIMD vector of `LANES` elements of type `Element`.
#[repr(simd)]
pub struct Simd<Element, const LANES: usize>([Element; LANES])
where
    Element: SimdElement,
    LaneCount<LANES>: SupportedLaneCount;

impl<Element, const LANES: usize> Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    /// Construct a SIMD vector by setting all lanes to the given value.
    pub const fn splat(value: Element) -> Self {
        Self([value; LANES])
    }

    /// Returns an array reference containing the entire SIMD vector.
    pub const fn as_array(&self) -> &[Element; LANES] {
        &self.0
    }

    /// Returns a mutable array reference containing the entire SIMD vector.
    pub fn as_mut_array(&mut self) -> &mut [Element; LANES] {
        &mut self.0
    }

    /// Converts an array to a SIMD vector.
    pub const fn from_array(array: [Element; LANES]) -> Self {
        Self(array)
    }

    /// Converts a SIMD vector to an array.
    pub const fn to_array(self) -> [Element; LANES] {
        self.0
    }

    /// SIMD gather: construct a SIMD vector by reading from a slice, using potentially discontiguous indices.
    /// If an index is out of bounds, that lane instead selects the value from the "or" vector.
    /// ```
    /// # #![feature(portable_simd)]
    /// # use core_simd::*;
    /// let vec: Vec<i32> = vec![10, 11, 12, 13, 14, 15, 16, 17, 18];
    /// let idxs = Simd::from_array([9, 3, 0, 5]);
    /// let alt = Simd::from_array([-5, -4, -3, -2]);
    ///
    /// let result = Simd::gather_or(&vec, idxs, alt); // Note the lane that is out-of-bounds.
    /// assert_eq!(result, Simd::from_array([-5, 13, 10, 15]));
    /// ```
    #[must_use]
    #[inline]
    pub fn gather_or(slice: &[Element], idxs: Simd<usize, LANES>, or: Self) -> Self {
        Self::gather_select(slice, Mask::splat(true), idxs, or)
    }

    /// SIMD gather: construct a SIMD vector by reading from a slice, using potentially discontiguous indices.
    /// Out-of-bounds indices instead use the default value for that lane (0).
    /// ```
    /// # #![feature(portable_simd)]
    /// # use core_simd::*;
    /// let vec: Vec<i32> = vec![10, 11, 12, 13, 14, 15, 16, 17, 18];
    /// let idxs = Simd::from_array([9, 3, 0, 5]);
    ///
    /// let result = Simd::gather_or_default(&vec, idxs); // Note the lane that is out-of-bounds.
    /// assert_eq!(result, Simd::from_array([0, 13, 10, 15]));
    /// ```
    #[must_use]
    #[inline]
    pub fn gather_or_default(slice: &[Element], idxs: Simd<usize, LANES>) -> Self
    where
        Element: Default,
    {
        Self::gather_or(slice, idxs, Self::splat(Element::default()))
    }

    /// SIMD gather: construct a SIMD vector by reading from a slice, using potentially discontiguous indices.
    /// Out-of-bounds or masked indices instead select the value from the "or" vector.
    /// ```
    /// # #![feature(portable_simd)]
    /// # use core_simd::*;
    /// let vec: Vec<i32> = vec![10, 11, 12, 13, 14, 15, 16, 17, 18];
    /// let idxs = Simd::from_array([9, 3, 0, 5]);
    /// let alt = Simd::from_array([-5, -4, -3, -2]);
    /// let mask = Mask::from_array([true, true, true, false]); // Note the mask of the last lane.
    ///
    /// let result = Simd::gather_select(&vec, mask, idxs, alt); // Note the lane that is out-of-bounds.
    /// assert_eq!(result, Simd::from_array([-5, 13, 10, -2]));
    /// ```
    #[must_use]
    #[inline]
    pub fn gather_select(
        slice: &[Element],
        mask: Mask<isize, LANES>,
        idxs: Simd<usize, LANES>,
        or: Self,
    ) -> Self {
        let mask = (mask & idxs.lanes_lt(Simd::splat(slice.len()))).to_int();
        let base_ptr = crate::vector::ptr::SimdConstPtr::splat(slice.as_ptr());
        // Ferris forgive me, I have done pointer arithmetic here.
        let ptrs = base_ptr.wrapping_add(idxs);
        // SAFETY: The ptrs have been bounds-masked to prevent memory-unsafe reads insha'allah
        unsafe { crate::intrinsics::simd_gather(or, ptrs, mask) }
    }

    /// SIMD scatter: write a SIMD vector's values into a slice, using potentially discontiguous indices.
    /// Out-of-bounds indices are not written.
    /// `scatter` writes "in order", so if an index receives two writes, only the last is guaranteed.
    /// ```
    /// # #![feature(portable_simd)]
    /// # use core_simd::*;
    /// let mut vec: Vec<i32> = vec![10, 11, 12, 13, 14, 15, 16, 17, 18];
    /// let idxs = Simd::from_array([9, 3, 0, 0]);
    /// let vals = Simd::from_array([-27, 82, -41, 124]);
    ///
    /// vals.scatter(&mut vec, idxs); // index 0 receives two writes.
    /// assert_eq!(vec, vec![124, 11, 12, 82, 14, 15, 16, 17, 18]);
    /// ```
    #[inline]
    pub fn scatter(self, slice: &mut [Element], idxs: Simd<usize, LANES>) {
        self.scatter_select(slice, Mask::splat(true), idxs)
    }

    /// SIMD scatter: write a SIMD vector's values into a slice, using potentially discontiguous indices.
    /// Out-of-bounds or masked indices are not written.
    /// `scatter_select` writes "in order", so if an index receives two writes, only the last is guaranteed.
    /// ```
    /// # #![feature(portable_simd)]
    /// # use core_simd::*;
    /// let mut vec: Vec<i32> = vec![10, 11, 12, 13, 14, 15, 16, 17, 18];
    /// let idxs = Simd::from_array([9, 3, 0, 0]);
    /// let vals = Simd::from_array([-27, 82, -41, 124]);
    /// let mask = Mask::from_array([true, true, true, false]); // Note the mask of the last lane.
    ///
    /// vals.scatter_select(&mut vec, mask, idxs); // index 0's second write is masked, thus omitted.
    /// assert_eq!(vec, vec![-41, 11, 12, 82, 14, 15, 16, 17, 18]);
    /// ```
    #[inline]
    pub fn scatter_select(
        self,
        slice: &mut [Element],
        mask: Mask<isize, LANES>,
        idxs: Simd<usize, LANES>,
    ) {
        // We must construct our scatter mask before we derive a pointer!
        let mask = (mask & idxs.lanes_lt(Simd::splat(slice.len()))).to_int();
        // SAFETY: This block works with *mut T derived from &mut 'a [T],
        // which means it is delicate in Rust's borrowing model, circa 2021:
        // &mut 'a [T] asserts uniqueness, so deriving &'a [T] invalidates live *mut Ts!
        // Even though this block is largely safe methods, it must be almost exactly this way
        // to prevent invalidating the raw ptrs while they're live.
        // Thus, entering this block requires all values to use being already ready:
        // 0. idxs we want to write to, which are used to construct the mask.
        // 1. mask, which depends on an initial &'a [T] and the idxs.
        // 2. actual values to scatter (self).
        // 3. &mut [T] which will become our base ptr.
        unsafe {
            // Now Entering ☢️ *mut T Zone
            let base_ptr = crate::vector::ptr::SimdMutPtr::splat(slice.as_mut_ptr());
            // Ferris forgive me, I have done pointer arithmetic here.
            let ptrs = base_ptr.wrapping_add(idxs);
            // The ptrs have been bounds-masked to prevent memory-unsafe writes insha'allah
            crate::intrinsics::simd_scatter(self, ptrs, mask)
            // Cleared ☢️ *mut T Zone
        }
    }
}

impl<Element, const LANES: usize> Copy for Simd<Element, LANES>
where
    Element: SimdElement,
    LaneCount<LANES>: SupportedLaneCount,
{
}

impl<Element, const LANES: usize> Clone for Simd<Element, LANES>
where
    Element: SimdElement,
    LaneCount<LANES>: SupportedLaneCount,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<Element, const LANES: usize> Default for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + Default,
{
    #[inline]
    fn default() -> Self {
        Self::splat(Element::default())
    }
}

impl<Element, const LANES: usize> PartialEq for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // TODO use SIMD equality
        self.to_array() == other.to_array()
    }
}

impl<Element, const LANES: usize> PartialOrd for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        // TODO use SIMD equality
        self.to_array().partial_cmp(other.as_ref())
    }
}

impl<Element, const LANES: usize> Eq for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + Eq,
{
}

impl<Element, const LANES: usize> Ord for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + Ord,
{
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // TODO use SIMD equality
        self.to_array().cmp(other.as_ref())
    }
}

impl<Element, const LANES: usize> core::hash::Hash for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement + core::hash::Hash,
{
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: core::hash::Hasher,
    {
        self.as_array().hash(state)
    }
}

// array references
impl<Element, const LANES: usize> AsRef<[Element; LANES]> for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    #[inline]
    fn as_ref(&self) -> &[Element; LANES] {
        &self.0
    }
}

impl<Element, const LANES: usize> AsMut<[Element; LANES]> for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    #[inline]
    fn as_mut(&mut self) -> &mut [Element; LANES] {
        &mut self.0
    }
}

// slice references
impl<Element, const LANES: usize> AsRef<[Element]> for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    #[inline]
    fn as_ref(&self) -> &[Element] {
        &self.0
    }
}

impl<Element, const LANES: usize> AsMut<[Element]> for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    #[inline]
    fn as_mut(&mut self) -> &mut [Element] {
        &mut self.0
    }
}

// vector/array conversion
impl<Element, const LANES: usize> From<[Element; LANES]> for Simd<Element, LANES>
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    fn from(array: [Element; LANES]) -> Self {
        Self(array)
    }
}

impl<Element, const LANES: usize> From<Simd<Element, LANES>> for [Element; LANES]
where
    LaneCount<LANES>: SupportedLaneCount,
    Element: SimdElement,
{
    fn from(vector: Simd<Element, LANES>) -> Self {
        vector.to_array()
    }
}

mod sealed {
    pub trait Sealed {}
}
use sealed::Sealed;

/// Marker trait for types that may be used as SIMD vector elements.
/// SAFETY: This trait, when implemented, asserts the compiler can monomorphize
/// `#[repr(simd)]` structs with the marked type as an element.
/// Strictly, it is valid to impl if the vector will not be miscompiled.
/// Practically, it is user-unfriendly to impl it if the vector won't compile,
/// even when no soundness guarantees are broken by allowing the user to try.
pub unsafe trait SimdElement: Sealed + Copy {
    /// The mask element type corresponding to this element type.
    type Mask: MaskElement;
}

impl Sealed for u8 {}
unsafe impl SimdElement for u8 {
    type Mask = i8;
}

impl Sealed for u16 {}
unsafe impl SimdElement for u16 {
    type Mask = i16;
}

impl Sealed for u32 {}
unsafe impl SimdElement for u32 {
    type Mask = i32;
}

impl Sealed for u64 {}
unsafe impl SimdElement for u64 {
    type Mask = i64;
}

impl Sealed for usize {}
unsafe impl SimdElement for usize {
    type Mask = isize;
}

impl Sealed for i8 {}
unsafe impl SimdElement for i8 {
    type Mask = i8;
}

impl Sealed for i16 {}
unsafe impl SimdElement for i16 {
    type Mask = i16;
}

impl Sealed for i32 {}
unsafe impl SimdElement for i32 {
    type Mask = i32;
}

impl Sealed for i64 {}
unsafe impl SimdElement for i64 {
    type Mask = i64;
}

impl Sealed for isize {}
unsafe impl SimdElement for isize {
    type Mask = isize;
}

impl Sealed for f32 {}
unsafe impl SimdElement for f32 {
    type Mask = i32;
}

impl Sealed for f64 {}
unsafe impl SimdElement for f64 {
    type Mask = i64;
}
